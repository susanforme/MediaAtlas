//! Automatic library refresh changes application data only. Every remote target
//! is derived from this scan's actual local files, never from a client-supplied ID.
use super::*;
use crate::scraper::automatic::{AutomaticContext, AutomaticOutcome};

impl ScanService {
    pub(super) async fn complete_automatic(
        &self,
        id: Uuid,
        snapshot: &RootSnapshot,
        cancel: &Arc<AtomicBool>,
        network_cancel: crate::network::CancellationToken,
        adult: Option<AdultAutomatic>,
    ) -> Result<(), RepositoryError> {
        self.phase(id, "index").await?;
        let mut after = String::new();
        let adult_access = adult.as_ref().map(|(_, access)| access);
        loop {
            if cancel.load(Ordering::Acquire) {
                return self.finish(id, "cancelled", None, None).await;
            }
            if let Some(access) = adult_access {
                access.ensure()?;
            }
            let ids: Vec<String> = sqlx::query_scalar("SELECT id FROM scan_candidates WHERE task_id=? AND status='pending' AND ((? AND kind='adult_video') OR (NOT ? AND kind<>'adult_video')) AND id>? ORDER BY id LIMIT 100")
                .bind(id.to_string()).bind(adult.is_some()).bind(adult.is_some()).bind(&after).fetch_all(self.repo.pool()).await?;
            if ids.is_empty() {
                break;
            }
            after = ids.last().cloned().unwrap_or_default();
            let candidates = ids
                .iter()
                .map(|s| Uuid::parse_str(s).map_err(|_| RepositoryError::CorruptData))
                .collect::<Result<Vec<_>, _>>()?;
            let input = ConfirmInput {
                task_id: id,
                candidate_ids: candidates.clone(),
            };
            if let Err(error) = self.confirm_inner(&input, true, adult_access).await {
                if !recoverable_file_change(&error) {
                    return Err(error);
                }
                // A changed/unreadable file must not discard the other valid
                // files in its page. Retain its pending record for a fresh scan.
                for candidate_id in candidates {
                    if cancel.load(Ordering::Acquire) {
                        break;
                    }
                    if let Err(error) = self
                        .confirm_inner(
                            &ConfirmInput {
                                task_id: id,
                                candidate_ids: vec![candidate_id],
                            },
                            true,
                            adult_access,
                        )
                        .await
                    {
                        if !recoverable_file_change(&error) {
                            return Err(error);
                        }
                        sqlx::query("UPDATE scan_sessions SET warnings=warnings+1 WHERE task_id=?")
                            .bind(id.to_string())
                            .execute(self.repo.pool())
                            .await?;
                    }
                }
            }
        }
        if cancel.load(Ordering::Acquire) {
            return self.finish(id, "cancelled", None, None).await;
        }
        let current = self.snapshot(snapshot.root.id).await?;
        if !same_config(snapshot, &current) || !current.root.enabled {
            return Err(RepositoryError::Conflict);
        }
        self.phase(id, "metadata").await?;
        // A series is enriched once even when hundreds of episodes were found.
        // Movie parts also share one target through the importer's stable group.
        let typed = crate::library_ready::typed_descendant("m", "child");
        let ancestor = crate::library_ready::ancestor_isolation("m");
        let targets_sql = if adult.is_some() {
            "INSERT OR IGNORE INTO library_refresh_items(task_id,media_item_id,state,updated_at) SELECT ?,m.id,'pending',? FROM media_items m WHERE m.adult_isolated=1 AND m.kind='adult_video' AND EXISTS(SELECT 1 FROM scan_candidates c WHERE c.task_id=? AND c.existing_media_id=m.id AND c.kind='adult_video' AND c.status IN ('imported','unchanged'))".to_owned()
        } else {
            format!("INSERT OR IGNORE INTO library_refresh_items(task_id,media_item_id,state,updated_at) SELECT ?,m.id,'pending',? FROM media_items m WHERE m.adult_isolated=0 AND ({ancestor}) AND m.kind IN ('movie','tv_series','anime') AND EXISTS(SELECT 1 FROM scan_candidates c JOIN media_items child ON child.id=c.existing_media_id WHERE c.task_id=? AND c.status IN ('imported','unchanged') AND (m.id=child.id OR ({typed})))")
        };
        sqlx::query(&targets_sql)
            .bind(id.to_string())
            .bind(Utc::now().timestamp_millis())
            .bind(id.to_string())
            .execute(self.repo.pool())
            .await?;
        let context = AutomaticContext {
            root: snapshot.root.clone(),
            exclusions: snapshot.exclude_patterns.clone(),
            task_id: id,
            cancel: network_cancel.clone(),
            adult_access: adult_access.cloned(),
        };
        loop {
            if cancel.load(Ordering::Acquire) || network_cancel.is_cancelled() {
                sqlx::query("UPDATE library_refresh_items SET state='needs_review',reason='cancelled',updated_at=? WHERE task_id=? AND state='pending'")
                    .bind(Utc::now().timestamp_millis()).bind(id.to_string()).execute(self.repo.pool()).await?;
                return self.finish(id, "cancelled", None, None).await;
            }
            let next: Option<String> = sqlx::query_scalar("SELECT media_item_id FROM library_refresh_items WHERE task_id=? AND state='pending' ORDER BY media_item_id LIMIT 1")
                .bind(id.to_string()).fetch_optional(self.repo.pool()).await?;
            let Some(media_id) = next else {
                break;
            };
            let media_id = Uuid::parse_str(&media_id).map_err(|_| RepositoryError::CorruptData)?;
            let enrichment = if let Some((scraper, _)) = &adult {
                Some(scraper.enrich_automatic(media_id, &context).await)
            } else if let Some(scraper) = &self.scraper {
                Some(scraper.enrich_automatic(media_id, &context).await)
            } else {
                None
            };
            let outcome = if let Some(result) = enrichment {
                match result {
                    Ok(outcome) => outcome,
                    Err(RepositoryError::Provider(
                        mediaatlas_domain::ProviderFailure::Cancelled,
                    )) if network_cancel.is_cancelled() => continue,
                    Err(RepositoryError::Provider(
                        mediaatlas_domain::ProviderFailure::Cancelled,
                    )) => AutomaticOutcome::review("cancelled"),
                    Err(_) => AutomaticOutcome::failed("metadata_failed"),
                }
            } else {
                AutomaticOutcome::review("provider_unconfigured")
            };
            // A manual identification can finish while this request is in
            // flight. Its newer result owns the row; only our own successful
            // merge may receive the subsequent artwork result annotation.
            sqlx::query("UPDATE library_refresh_items SET state=?,reason=?,run_id=?,updated_at=? WHERE task_id=? AND media_item_id=? AND (state='pending' OR (state='matched' AND run_id=?))")
                .bind(outcome.state).bind(outcome.reason).bind(outcome.run_id.map(|v|v.to_string())).bind(Utc::now().timestamp_millis()).bind(id.to_string()).bind(media_id.to_string()).bind(outcome.run_id.map(|v|v.to_string())).execute(self.repo.pool()).await?;
            self.refresh_counts(id).await?;
            self.emit(id).await;
        }
        let pending: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM scan_candidates WHERE task_id=? AND status='pending'",
        )
        .bind(id.to_string())
        .fetch_one(self.repo.pool())
        .await?;
        self.phase(id, "complete").await?;
        self.refresh_counts(id).await?;
        let task = self.get_task(id).await?;
        self.finish(
            id,
            if pending > 0 || task.review_count > 0 || task.metadata_failed_count > 0 {
                "waiting_for_user"
            } else {
                "succeeded"
            },
            if pending > 0 {
                Some("files_changed")
            } else {
                None
            },
            Some(snapshot.root.id),
        )
        .await
    }

    async fn phase(&self, id: Uuid, phase: &str) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE scan_sessions SET phase=? WHERE task_id=?")
            .bind(phase)
            .bind(id.to_string())
            .execute(self.repo.pool())
            .await?;
        self.emit(id).await;
        Ok(())
    }

    async fn refresh_counts(&self, id: Uuid) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE scan_sessions SET matched_count=(SELECT count(*) FROM library_refresh_items WHERE task_id=? AND state='matched'),review_count=(SELECT count(*) FROM library_refresh_items WHERE task_id=? AND state='needs_review'),metadata_failed_count=(SELECT count(*) FROM library_refresh_items WHERE task_id=? AND state='failed') WHERE task_id=?")
            .bind(id.to_string()).bind(id.to_string()).bind(id.to_string()).bind(id.to_string()).execute(self.repo.pool()).await?;
        Ok(())
    }
}

fn recoverable_file_change(error: &RepositoryError) -> bool {
    matches!(
        error,
        RepositoryError::Conflict | RepositoryError::InvalidInput | RepositoryError::NotFound
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn infrastructure_failures_are_not_mislabeled_as_changed_files() {
        assert!(recoverable_file_change(&RepositoryError::Conflict));
        assert!(!recoverable_file_change(&RepositoryError::Database));
        assert!(!recoverable_file_change(&RepositoryError::CorruptData));
        assert!(!recoverable_file_change(
            &RepositoryError::ProtectedMetadata
        ));
    }
}
