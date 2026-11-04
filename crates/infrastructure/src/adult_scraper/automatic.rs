//! Catalog-only automatic enrichment for an explicitly unlocked adult Root.
use super::*;
use crate::scraper::automatic::{AutomaticContext, AutomaticOutcome};

fn automatic_candidate(query: &AdultQuery, candidates: &[AdultMatchedCandidate]) -> Option<usize> {
    let number = normalize_catalog_number(&query.catalog_number, &query.namespace).ok()?;
    let eligible = candidates
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            let catalog = &item.candidate.catalog_number;
            !item.needs_review
                && catalog.normalized == number.normalized
                && catalog.namespace == number.namespace
                && number
                    .part
                    .as_ref()
                    .zip(catalog.part.as_ref())
                    .is_none_or(|(a, b)| a == b)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    (eligible.len() == 1).then(|| eligible[0])
}

fn query_from_local(values: &[MetadataValue], ids: &[ExternalId]) -> Option<AdultQuery> {
    let mut scoped = vec![];
    for id in ids {
        if id.namespace == "dmm.product" || id.namespace.starts_with("wikidata.catalog:") {
            scoped.push(normalize_catalog_number(&id.external_id, &id.namespace).ok()?);
        }
    }
    for value in values.iter().filter(|v| v.field_key == "catalog_numbers") {
        for number in value.value.as_array()? {
            let number: CatalogNumber = serde_json::from_value(number.clone()).ok()?;
            let normalized = normalize_catalog_number(&number.raw, &number.namespace).ok()?;
            if normalized.normalized != number.normalized {
                return None;
            }
            scoped.push(normalized);
        }
    }
    if scoped.is_empty() {
        // A parser number is a search term in the selected DMM adapter's product
        // namespace, not a global issuer claim. Only an exact DMM result can win.
        for value in values.iter().filter(|v| v.field_key == "catalog_number") {
            scoped.push(normalize_catalog_number(value.value.as_str()?, "dmm.product").ok()?);
        }
    }
    let first = scoped.first()?;
    if scoped.iter().any(|n| {
        n.normalized != first.normalized || n.namespace != first.namespace || n.part != first.part
    }) {
        return None;
    }
    Some(AdultQuery {
        catalog_number: first.raw.clone(),
        namespace: first.namespace.clone(),
        studio: values
            .iter()
            .find(|v| v.field_key == "studio")
            .and_then(|v| v.value.as_str())
            .filter(|v| !v.trim().is_empty())
            .map(str::to_owned),
        language: "zh-CN".into(),
    })
}

impl AdultScraperService {
    pub(crate) async fn enrich_automatic(
        &self,
        media_id: Uuid,
        context: &AutomaticContext,
    ) -> Result<AutomaticOutcome> {
        // Automatic work never uses the legacy test-only unscoped constructor.
        context
            .adult_access
            .as_ref()
            .ok_or(RepositoryError::InvalidInput)?
            .ensure()?;
        self.access
            .as_ref()
            .ok_or(RepositoryError::InvalidInput)?
            .ensure()?;
        let mut tx = self.repo.pool().begin().await?;
        context.validate_tx(&mut tx, media_id).await?;
        tx.commit().await?;
        let media = self.adult_media(media_id).await?;
        let established: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM local_metadata_provenance WHERE media_item_id=?) OR EXISTS(SELECT 1 FROM adult_metadata_sources WHERE media_item_id=?)")
            .bind(media_id.to_string()).bind(media_id.to_string()).fetch_one(self.repo.pool()).await?;
        if established {
            let mut tx = self.repo.pool().begin().await?;
            context.validate_tx(&mut tx, media_id).await?;
            tx.commit().await?;
            let _ = self
                .artwork_service()
                .automatic_nfo(media_id, Some(&context.cancel), Some(context))
                .await;
            return Ok(AutomaticOutcome {
                state: "skipped",
                reason: Some("existing_metadata"),
                run_id: None,
            });
        }
        if context.metadata_providers.as_ref().is_some_and(Vec::is_empty) {
            return Ok(AutomaticOutcome { state: "skipped", reason: Some("local_only"), run_id: None });
        }
        let values = self.repo.list_metadata_values(media_id).await?;
        let ids = self.repo.list_external_ids(media_id).await?;
        let Some(query) = query_from_local(&values, &ids) else {
            return Ok(AutomaticOutcome::review("catalog_unrecognized"));
        };
        let provider = if query.namespace == "dmm.product" {
            "dmm"
        } else if query.namespace.starts_with("wikidata.catalog:") {
            "wikidata"
        } else {
            return Ok(AutomaticOutcome::review("issuer_unconfigured"));
        };
        if let Some(providers) = &context.metadata_providers {
            if !providers.iter().any(|selected| selected == provider) {
                return Ok(AutomaticOutcome { state: "skipped", reason: Some("local_only"), run_id: None });
            }
        }
        let config = self.providers.config(provider).await?;
        if !config.enabled
            || (provider == "dmm" && self.auth.is_none())
            || if let Some(auth) = &self.auth {
                !auth.status(provider).await?.configured
            } else {
                false
            }
        {
            return Ok(AutomaticOutcome::review("provider_unconfigured"));
        }
        self.ensure_access()?;
        if context.cancel.is_cancelled() {
            return Err(RepositoryError::Provider(ProviderFailure::Cancelled));
        }
        let search = self
            .search_cancellable(
                AdultSearchRequest {
                    query: query.clone(),
                    providers: vec![provider.into()],
                },
                context.cancel.clone(),
            )
            .await?;
        if let Some(failure) = search.failures.first() {
            return Ok(match failure.failure {
                ProviderFailure::CredentialsUnavailable | ProviderFailure::Disabled => {
                    AutomaticOutcome::review("provider_unconfigured")
                }
                error => AutomaticOutcome::failed(error.code()),
            }
            .with_run(search.run_id));
        }
        let Some(index) = automatic_candidate(&query, &search.candidates) else {
            return Ok(AutomaticOutcome::review(if search.candidates.is_empty() {
                "no_match"
            } else {
                "ambiguous_match"
            })
            .with_run(search.run_id));
        };
        let detail = self
            .detail_cancellable(search.run_id, index, context.cancel.clone())
            .await?;
        if self.repo.get_media(media_id).await?.as_ref() != Some(&media) {
            return Err(RepositoryError::Conflict);
        }
        let protected = self
            .repo
            .list_metadata_values(media_id)
            .await?
            .into_iter()
            .filter(MetadataValue::is_protected)
            .map(|v| v.field_key)
            .collect::<HashSet<_>>();
        let selected_fields = adult_fields(&detail)?
            .into_iter()
            .filter(|f| !protected.contains(&f.key))
            .map(|f| f.key)
            .collect();
        let existing = self.repo.list_artworks(media_id).await?;
        let selected_artwork =
            if self.automatic_artwork && !existing.iter().any(|a| a.kind.as_str() == "cover") {
                detail
                    .artwork
                    .iter()
                    .position(|a| a.kind.as_str() == "cover")
                    .into_iter()
                    .collect()
            } else {
                vec![]
            };
        let preview = self
            .preview(AdultMergeRequest {
                run_id: search.run_id,
                candidate_index: index,
                media_item_id: media_id,
                selected_fields,
                selected_artwork,
            })
            .await?;
        self.confirm_inner(preview.preview_id, Some(context))
            .await?;
        Ok(AutomaticOutcome {
            state: "matched",
            reason: None,
            run_id: Some(search.run_id),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalog_identity_is_unique_and_issuer_scoped() {
        let query = AdultQuery {
            catalog_number: "SYN-001".into(),
            namespace: "dmm.product".into(),
            studio: None,
            language: "en-US".into(),
        };
        let candidate = |raw: &str, namespace: &str| {
            match_adult_candidate(
                &query,
                AdultCandidate {
                    provider_id: "dmm".into(),
                    remote_id: raw.into(),
                    catalog_number: normalize_catalog_number(raw, namespace).unwrap(),
                    title: "Synthetic title".into(),
                    studio: None,
                    source_url: "https://example.invalid".into(),
                    language: "en-US".into(),
                },
                &[],
            )
        };
        assert_eq!(
            automatic_candidate(&query, &[candidate("SYN-001", "dmm.product")]),
            Some(0)
        );
        assert_eq!(
            automatic_candidate(
                &query,
                &[
                    candidate("SYN-002", "dmm.product"),
                    candidate("SYN-001", "dmm.product")
                ]
            ),
            Some(1)
        );
        assert_eq!(
            automatic_candidate(&query, &[candidate("SYN-001", "wikidata.catalog:Q1")]),
            None
        );
        assert_eq!(
            automatic_candidate(
                &query,
                &[
                    candidate("SYN-001", "dmm.product"),
                    candidate("SYN-001", "dmm.product")
                ]
            ),
            None
        );
    }
}
