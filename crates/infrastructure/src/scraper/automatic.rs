//! Automatic metadata enrichment is a separate policy from manual review. It
//! accepts only one strong identity match, then exports absent NFO sidecars.
use super::*;
use sqlx::{Sqlite, Transaction};

pub(crate) struct AutomaticContext {
    pub root: LibraryRoot,
    pub exclusions: Vec<String>,
    pub metadata_providers: Option<Vec<String>>,
    pub nfo_export_mode: NfoExportMode,
    pub task_id: Uuid,
    pub cancel: CancellationToken,
    pub adult_access: Option<crate::media_output::AdultOutputAccess>,
}

impl AutomaticContext {
    pub(crate) async fn validate_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        media_id: Uuid,
    ) -> Result<(), RepositoryError> {
        if self.cancel.is_cancelled() {
            return Err(RepositoryError::Provider(ProviderFailure::Cancelled));
        }
        if let Some(access) = &self.adult_access {
            access.ensure()?;
        }
        // The writer reservation serializes final publication against root
        // edits, cancellation and privacy reclassification.
        sqlx::query("UPDATE library_roots SET updated_at=updated_at WHERE id=?")
            .bind(self.root.id.to_string())
            .execute(&mut **tx)
            .await?;
        let root = sqlx::query("SELECT r.*,s.exclude_patterns_json,s.metadata_providers_json,s.nfo_export_mode FROM library_roots r LEFT JOIN root_settings s ON s.root_id=r.id WHERE r.id=?")
            .bind(self.root.id.to_string()).fetch_optional(&mut **tx).await?.ok_or(RepositoryError::NotFound)?;
        let exclusions: Vec<String> = serde_json::from_str(
            root.try_get::<Option<String>, _>("exclude_patterns_json")?
                .as_deref()
                .unwrap_or("[]"),
        )
        .map_err(|_| RepositoryError::CorruptData)?;
        let providers: Option<Vec<String>> = root.try_get::<Option<String>, _>("metadata_providers_json")?
            .map(|value| serde_json::from_str(&value).map_err(|_| RepositoryError::CorruptData)).transpose()?;
        let nfo_export_mode = crate::root_sources::parse_nfo_export_mode(
            root.try_get::<Option<String>, _>("nfo_export_mode")?.as_deref().unwrap_or("online"),
        )?;
        if !root.try_get::<bool, _>("enabled")?
            || root.try_get::<String, _>("path")? != self.root.path
            || root.try_get::<String, _>("path_style")? != self.root.path_style.as_str()
            || root.try_get::<Option<String>, _>("kind")?
                != self.root.kind.map(|k| k.as_str().to_owned())
            || (self.root.kind == Some(MediaKind::AdultVideo)) != self.adult_access.is_some()
            || exclusions != self.exclusions
            || providers != self.metadata_providers
            || nfo_export_mode != self.nfo_export_mode
        {
            return Err(RepositoryError::Conflict);
        }
        let running: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM scrape_tasks t JOIN scan_sessions s ON s.task_id=t.id WHERE t.id=? AND t.state='running' AND s.mode='automatic' AND s.root_id=?)")
            .bind(self.task_id.to_string()).bind(self.root.id.to_string()).fetch_one(&mut **tx).await?;
        let typed = crate::library_ready::typed_descendant("m", "child");
        let ancestor = crate::library_ready::ancestor_isolation("m");
        let child_ancestor = crate::library_ready::ancestor_isolation("child");
        let sql = if self.adult_access.is_some() {
            "SELECT EXISTS(SELECT 1 FROM media_items m WHERE m.id=? AND m.adult_isolated=1 AND m.kind='adult_video' AND NOT EXISTS(SELECT 1 FROM media_files mf JOIN library_roots mr ON mr.id=mf.root_id WHERE mf.media_item_id=m.id AND (mr.kind IS NULL OR mr.kind<>'adult_video' OR mr.enabled=0)) AND EXISTS(SELECT 1 FROM scan_candidates c JOIN media_files f ON f.media_item_id=m.id AND f.root_id=c.root_id AND f.normalized_relative_path=c.normalized_relative_path WHERE c.task_id=? AND c.existing_media_id=m.id AND c.kind='adult_video' AND c.status IN ('imported','unchanged')))".to_owned()
        } else {
            format!("SELECT EXISTS(SELECT 1 FROM media_items m WHERE m.id=? AND m.adult_isolated=0 AND m.kind<>'adult_video' AND ({ancestor}) AND NOT EXISTS(SELECT 1 FROM media_files mf JOIN library_roots mr ON mr.id=mf.root_id WHERE mf.media_item_id=m.id AND (mr.kind='adult_video' OR mr.enabled=0)) AND EXISTS(SELECT 1 FROM scan_candidates c JOIN media_items child ON child.id=c.existing_media_id JOIN media_files f ON f.media_item_id=child.id AND f.root_id=c.root_id AND f.normalized_relative_path=c.normalized_relative_path WHERE c.task_id=? AND c.status IN ('imported','unchanged') AND child.adult_isolated=0 AND child.kind<>'adult_video' AND ({child_ancestor}) AND NOT EXISTS(SELECT 1 FROM media_files cf JOIN library_roots cr ON cr.id=cf.root_id WHERE cf.media_item_id=child.id AND (cr.kind='adult_video' OR cr.enabled=0)) AND (m.id=child.id OR ({typed}))))")
        };
        let target: bool = sqlx::query_scalar(&sql)
            .bind(media_id.to_string())
            .bind(self.task_id.to_string())
            .fetch_one(&mut **tx)
            .await?;
        if !running || !target || self.cancel.is_cancelled() {
            return Err(RepositoryError::Conflict);
        }
        if let Some(access) = &self.adult_access {
            access.ensure()?;
        }
        Ok(())
    }
}

pub(crate) struct AutomaticOutcome {
    pub state: &'static str,
    pub reason: Option<&'static str>,
    pub run_id: Option<Uuid>,
}
impl AutomaticOutcome {
    pub fn review(reason: &'static str) -> Self {
        Self {
            state: "needs_review",
            reason: Some(reason),
            run_id: None,
        }
    }
    pub fn failed(reason: &'static str) -> Self {
        Self {
            state: "failed",
            reason: Some(reason),
            run_id: None,
        }
    }
    pub(crate) fn with_run(mut self, run_id: Uuid) -> Self {
        self.run_id = Some(run_id);
        self
    }
}

/// A unique exact title/alias or external ID is sufficient. A known local year
/// must agree; a yearless title with multiple exact results remains ambiguous.
/// Position in provider results is never evidence.
fn automatic_candidate(query: &VideoQuery, candidates: &[MatchedCandidate]) -> Option<usize> {
    let (_, year, _, _) = query.parts();
    let strong = candidates
        .iter()
        .enumerate()
        .filter(|(_, candidate)| {
            let reasons = &candidate.reasons;
            let has = |reason: &str| reasons.iter().any(|r| r == reason);
            candidate.candidate.kind() == query.kind()
                && !has("external_id_conflict")
                && !has("year_conflict")
                && (has("external_id_exact")
                    || ((has("title_exact") || has("anime_alias_exact"))
                        && year.is_none_or(|year| Some(year) == candidate.candidate.data().year)))
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    (strong.len() == 1).then(|| strong[0])
}

impl ScraperService {
    pub(crate) async fn enrich_automatic(
        &self,
        media_id: Uuid,
        context: &AutomaticContext,
    ) -> Result<AutomaticOutcome, RepositoryError> {
        if context.adult_access.is_some() {
            return Err(RepositoryError::InvalidInput);
        }
        let mut tx = self.repo.pool().begin().await?;
        context.validate_tx(&mut tx, media_id).await?;
        tx.commit().await?;
        let media = self
            .repo
            .get_media(media_id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let typed = crate::library_ready::typed_descendant("parent", "m");
        let local_metadata: bool = sqlx::query_scalar(&format!(
            "SELECT EXISTS(SELECT 1 FROM local_metadata_provenance WHERE media_item_id=?) OR EXISTS(SELECT 1 FROM media_items m JOIN media_items parent ON ({typed}) JOIN local_metadata_provenance p ON p.media_item_id=parent.id WHERE m.id=? AND p.source_kind='nfo')",
        ))
        .bind(media_id.to_string()).bind(media_id.to_string())
        .fetch_one(self.repo.pool())
        .await?;
        let saved: Option<String> = sqlx::query_scalar(
            "SELECT metadata_json FROM video_metadata_sources WHERE media_item_id=?",
        )
        .bind(media_id.to_string())
        .fetch_optional(self.repo.pool())
        .await?;
        let existing: Option<VideoMetadata> = saved.map(|value| decode(&value)).transpose()?;
        let existing_art = self.repo.list_artworks(media_id).await?;
        let repair_artwork = existing.as_ref().is_some_and(|detail| {
            detail.artwork().iter().any(|art| {
                matches!(art.kind.as_str(), "poster" | "backdrop")
                    && !existing_art.iter().any(|old| old.kind.as_str() == art.kind)
            })
        });
        if local_metadata || existing.is_some() {
            let mut tx = self.repo.pool().begin().await?;
            context.validate_tx(&mut tx, media_id).await?;
            tx.commit().await?;
            let output =
                crate::media_output::MediaOutputService::new(self.repo.clone(), self.cache.clone())
                    .with_network_engine(self.engine.clone());
            let _ = output
                .automatic_nfo(media_id, Some(&context.cancel), Some(context))
                .await;
            // Restore NFOs from saved metadata even when remote artwork repair
            // will be unavailable or still needs review.
            if local_metadata || !repair_artwork {
                return Ok(AutomaticOutcome {
                    state: "skipped",
                    reason: Some("existing_metadata"),
                    run_id: None,
                });
            }
        }
        if !matches!(
            media.kind,
            MediaKind::Movie | MediaKind::TvSeries | MediaKind::Anime
        ) {
            return Ok(AutomaticOutcome::review("unsupported_kind"));
        }
        let providers = context.metadata_providers.clone()
            .unwrap_or_else(|| crate::root_sources::default_providers(media.kind));
        if providers.is_empty() {
            return Ok(AutomaticOutcome {
                state: "skipped",
                reason: Some("local_only"),
                run_id: None,
            });
        }
        let ids = self
            .repo
            .list_external_ids(media_id)
            .await?
            .into_iter()
            .map(|id| RemoteId {
                namespace: id.namespace,
                value: id.external_id,
            })
            .collect::<Vec<_>>();
        let language = "zh-CN".to_owned();
        let query = match media.kind {
            MediaKind::Movie => VideoQuery::Movie {
                title: media.title.clone(),
                year: media.year,
                external_ids: ids,
                language,
            },
            MediaKind::TvSeries => VideoQuery::Tv {
                title: media.title.clone(),
                year: media.year,
                external_ids: ids,
                language,
            },
            MediaKind::Anime => VideoQuery::Anime {
                title: media.title.clone(),
                year: media.year,
                external_ids: ids,
                language,
            },
            _ => unreachable!(),
        };
        let mut selected = None;
        let mut unavailable = AutomaticOutcome::review("provider_unconfigured");
        for provider in providers {
            if !VideoProviderHub::descriptors().iter().any(|descriptor| {
                descriptor.id == provider && descriptor.media_kinds.contains(&media.kind)
            }) {
                continue;
            }
            let config = self.providers.config(&provider).await?;
            if !config.enabled
                || if let Some(auth) = &self.auth {
                    !auth.status(&provider).await?.configured
                } else {
                    false
                }
            {
                continue;
            }
            let search = self.search_cancellable(
                ScraperSearchRequest { query: query.clone(), providers: vec![provider] },
                context.cancel.clone(),
            ).await?;
            if context.cancel.is_cancelled() {
                let _ = self.providers.cancel(search.run_id).await;
                return Err(RepositoryError::Provider(ProviderFailure::Cancelled));
            }
            if let Some(failure) = search.failures.first() {
                unavailable = match failure.failure {
                    ProviderFailure::CredentialsUnavailable | ProviderFailure::Disabled => {
                        AutomaticOutcome::review("provider_unconfigured")
                    }
                    other => AutomaticOutcome::failed(other.code()),
                }.with_run(search.run_id);
                continue;
            }
            let Some(index) = automatic_candidate(&query, &search.candidates) else {
                if !search.candidates.is_empty() {
                    return Ok(AutomaticOutcome::review("ambiguous_match").with_run(search.run_id));
                }
                unavailable = AutomaticOutcome::review("no_match").with_run(search.run_id);
                continue;
            };
            if existing.as_ref().is_some_and(|saved| {
                saved.identity().provider_id != search.candidates[index].candidate.data().provider_id
                    || saved.identity().remote_id != search.candidates[index].candidate.data().remote_id
            }) {
                return Ok(AutomaticOutcome::review("identity_changed").with_run(search.run_id));
            }
            match self.detail_cancellable(search.run_id, index, context.cancel.clone()).await {
                Ok(detail) => {
                    selected = Some((search, index, detail));
                    break;
                }
                Err(RepositoryError::Provider(ProviderFailure::Cancelled)) => {
                    return Err(RepositoryError::Provider(ProviderFailure::Cancelled));
                }
                Err(RepositoryError::Provider(failure)) => {
                    unavailable = AutomaticOutcome::failed(failure.code()).with_run(search.run_id);
                }
                Err(error) => return Err(error),
            }
        }
        let Some((search, index, detail)) = selected else {
            return Ok(unavailable);
        };
        if self.repo.get_media(media_id).await?.as_ref() != Some(&media) {
            return Err(RepositoryError::Conflict);
        }
        let protected = self
            .repo
            .list_metadata_values(media_id)
            .await?
            .into_iter()
            .filter(|v| v.is_protected())
            .map(|v| v.field_key)
            .collect::<HashSet<_>>();
        let selected_fields = detail
            .fields()
            .iter()
            .filter(|f| !repair_artwork && !protected.contains(&f.key))
            .map(|f| f.key.clone())
            .collect();
        // At most one poster and one backdrop are copied into the app cache.
        // Existing local art remains preferred; only absent NFOs are exported later.
        let mut selected_artwork = vec![];
        for kind in ["poster", "backdrop"] {
            if !existing_art.iter().any(|art| art.kind.as_str() == kind) {
                if let Some(index) = detail
                    .artwork()
                    .iter()
                    .position(|art| art.kind.as_str() == kind)
                {
                    selected_artwork.push(index);
                }
            }
        }
        let request = MergeRequest {
            run_id: search.run_id,
            candidate_index: index,
            media_item_id: media_id,
            selected_fields,
            selected_artwork,
        };
        let preview = self.preview(request.clone()).await?;
        if context.cancel.is_cancelled() {
            return Err(RepositoryError::Provider(ProviderFailure::Cancelled));
        }
        let result = self.confirm_inner(preview.preview_id, Some(context)).await;
        let artwork_failed = match result {
            Ok(_) => false,
            Err(RepositoryError::Provider(ProviderFailure::Cancelled)) => {
                return Err(RepositoryError::Provider(ProviderFailure::Cancelled))
            }
            Err(RepositoryError::Provider(_))
                if !request.selected_artwork.is_empty() && !context.cancel.is_cancelled() =>
            {
                // A broken image must not discard valid matching metadata. The
                // fallback still performs the complete protected merge checks.
                let fallback = self
                    .preview(MergeRequest {
                        selected_artwork: vec![],
                        ..request
                    })
                    .await?;
                self.confirm_inner(fallback.preview_id, Some(context))
                    .await?;
                true
            }
            Err(error) => return Err(error),
        };
        Ok(AutomaticOutcome {
            state: if artwork_failed { "failed" } else { "matched" },
            reason: artwork_failed.then_some("artwork_failed"),
            run_id: Some(search.run_id),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn query(year: Option<i32>) -> VideoQuery {
        VideoQuery::Movie {
            title: "Example".into(),
            year,
            external_ids: vec![],
            language: "en-US".into(),
        }
    }
    fn candidate(year: Option<i32>, id: &str) -> MatchedCandidate {
        let data = CandidateData {
            provider_id: "tmdb".into(),
            remote_id: id.into(),
            title: "Example".into(),
            original_title: None,
            year,
            aliases: vec![],
            external_ids: vec![],
            source_url: "https://example.invalid".into(),
            language: "en-US".into(),
        };
        match_candidate(&query(Some(2020)), VideoCandidate::Movie(data), &[])
    }
    #[test]
    fn automatic_identity_requires_one_strong_match_not_first_result() {
        assert_eq!(
            automatic_candidate(
                &query(Some(2020)),
                &[candidate(Some(1980), "old"), candidate(Some(2020), "new")]
            ),
            Some(1)
        );
        assert_eq!(
            automatic_candidate(&query(None), &[candidate(Some(2020), "new")]),
            Some(0)
        );
        let mut other_year = candidate(Some(1980), "old");
        other_year.reasons.retain(|r| r != "year_conflict");
        assert_eq!(
            automatic_candidate(&query(None), &[candidate(Some(2020), "new"), other_year]),
            None
        );
        assert_eq!(
            automatic_candidate(
                &query(Some(2020)),
                &[candidate(Some(2020), "a"), candidate(Some(2020), "b")]
            ),
            None
        );
        assert_eq!(
            automatic_candidate(&query(Some(2020)), &[candidate(None, "unknown")]),
            None
        );
        let mut exact = candidate(None, "exact");
        exact.reasons = vec!["external_id_exact".into()];
        assert_eq!(automatic_candidate(&query(None), &[exact.clone()]), Some(0));
        exact.reasons.push("external_id_conflict".into());
        assert_eq!(automatic_candidate(&query(None), &[exact]), None);
    }
}
