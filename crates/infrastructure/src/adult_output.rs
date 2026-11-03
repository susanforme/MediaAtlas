//! Independently scoped, explicit adult output batches over immutable safe plans.
use crate::{
    library::{LibraryRepository, RepositoryError},
    media_output::{MediaOutputService, OutputPreview, OutputResult},
    network::CancellationToken,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, OnceLock},
};
use tokio::sync::Mutex;
use uuid::Uuid;

const MAX_BATCH_SNAPSHOT_BYTES: i64 = 128 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdultBatchPreview {
    pub schema_version: u16,
    pub batch_id: Uuid,
    pub state: String,
    pub items: Vec<OutputPreview>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdultBatchItemResult {
    pub media_item_id: Uuid,
    pub preview_id: Uuid,
    pub state: String,
    pub result: Option<OutputResult>,
    pub error_code: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdultBatchResult {
    pub schema_version: u16,
    pub batch_id: Uuid,
    pub state: String,
    pub items: Vec<AdultBatchItemResult>,
}
fn active_batches() -> Arc<Mutex<HashMap<Uuid, CancellationToken>>> {
    static ACTIVE: OnceLock<Arc<Mutex<HashMap<Uuid, CancellationToken>>>> = OnceLock::new();
    ACTIVE
        .get_or_init(|| Arc::new(Mutex::new(HashMap::new())))
        .clone()
}
/// Startup recovery needs no unlock grant and never reads or writes source files.
pub async fn recover_adult_batches(repo: &LibraryRepository) -> Result<(), RepositoryError> {
    let mut tx = repo.pool().begin().await?;
    sqlx::query("UPDATE scrape_tasks SET state='interrupted',updated_at=MAX(updated_at,?) WHERE id IN (SELECT task_id FROM adult_output_batches WHERE state='running')")
        .bind(Utc::now().timestamp_millis()).execute(&mut *tx).await?;
    sqlx::query("UPDATE adult_output_batches SET state='interrupted' WHERE state='running'")
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE adult_output_batch_items SET state='interrupted' WHERE state='running'")
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(())
}
#[derive(Clone)]
pub struct AdultBatchService {
    repo: LibraryRepository,
    output: MediaOutputService,
    active: Arc<Mutex<HashMap<Uuid, CancellationToken>>>,
}
impl AdultBatchService {
    pub fn new(repo: LibraryRepository, output: MediaOutputService) -> Self {
        Self {
            repo,
            output,
            active: active_batches(),
        }
    }
    /// Recovery records interruption; it never performs or schedules file I/O.
    pub async fn initialize(&self) -> Result<(), RepositoryError> {
        recover_adult_batches(&self.repo).await
    }
    pub async fn preview(
        &self,
        media_ids: Vec<Uuid>,
    ) -> Result<AdultBatchPreview, RepositoryError> {
        self.output.ensure_access()?;
        if !self.output.is_adult_channel()
            || media_ids.is_empty()
            || media_ids.len() > 32
            || media_ids.iter().collect::<HashSet<_>>().len() != media_ids.len()
        {
            return Err(RepositoryError::InvalidInput);
        }
        let mut items: Vec<OutputPreview> = Vec::new();
        let mut total_snapshot_bytes = 0_i64;
        for id in media_ids {
            match self.output.preview(id).await {
                Ok(item) => items.push(item),
                Err(error) => {
                    for item in items {
                        let _ = self.output.discard(item.preview_id).await;
                    }
                    return Err(error);
                }
            }
            let size: Result<i64, RepositoryError> = sqlx::query_scalar(
                "SELECT length(CAST(snapshot AS BLOB)) FROM output_previews WHERE id=?",
            )
            .bind(
                items
                    .last()
                    .ok_or(RepositoryError::CorruptData)?
                    .preview_id
                    .to_string(),
            )
            .fetch_one(self.repo.pool())
            .await
            .map_err(Into::into);
            let budget_result = size.and_then(|size| {
                total_snapshot_bytes = total_snapshot_bytes.saturating_add(size);
                if total_snapshot_bytes > MAX_BATCH_SNAPSHOT_BYTES {
                    Err(RepositoryError::Provider(
                        mediaatlas_domain::ProviderFailure::ResponseTooLarge,
                    ))
                } else {
                    Ok(())
                }
            });
            if let Err(error) = budget_result {
                for item in &items {
                    let _ = self.output.discard(item.preview_id).await;
                }
                return Err(error);
            }
        }
        let preview = AdultBatchPreview {
            schema_version: 1,
            batch_id: Uuid::new_v4(),
            state: "planned".into(),
            items,
        };
        let persisted: Result<(),RepositoryError> = async {
        self.output.ensure_access()?;
        let now = Utc::now().timestamp_millis();
        let mut tx = self.repo.pool().begin().await?;
        sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES (?,'export_nfo','waiting_for_user',?,?,?)")
            .bind(preview.batch_id.to_string())
            .bind(serde_json::json!({"adult":true,"channel":"adult","batch_id":preview.batch_id}).to_string())
            .bind(now).bind(now).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO adult_output_batches(id,task_id,snapshot,state,created_at,updated_at) VALUES (?,?,?,'planned',?,?)")
            .bind(preview.batch_id.to_string()).bind(preview.batch_id.to_string())
            .bind(serde_json::to_string(&preview).map_err(|_| RepositoryError::InvalidInput)?)
            .bind(now).bind(now).execute(&mut *tx).await?;
        for (ordinal, item) in preview.items.iter().enumerate() {
            sqlx::query("INSERT INTO adult_output_batch_items(batch_id,ordinal,media_item_id,preview_id,state) VALUES (?,?,?,?,'planned')")
                .bind(preview.batch_id.to_string()).bind(ordinal as i64).bind(item.media_item_id.to_string()).bind(item.preview_id.to_string())
                .execute(&mut *tx).await?;
            sqlx::query("UPDATE file_operations SET task_id=? WHERE id=?")
                .bind(preview.batch_id.to_string())
                .bind(item.preview_id.to_string())
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        self.output.ensure_access()?;
        Ok(())
        }.await;
        if let Err(error) = persisted {
            for item in &preview.items {
                let _ = self.output.discard(item.preview_id).await;
            }
            return Err(error);
        }
        Ok(preview)
    }
    pub async fn status(&self, batch_id: Uuid) -> Result<AdultBatchResult, RepositoryError> {
        self.output.ensure_access()?;
        if !self.output.is_adult_channel() {
            return Err(RepositoryError::InvalidInput);
        }
        let result = self.status_inner(batch_id).await?;
        self.output.ensure_access()?;
        Ok(result)
    }
    async fn status_inner(&self, batch_id: Uuid) -> Result<AdultBatchResult, RepositoryError> {
        let state: String = sqlx::query_scalar("SELECT state FROM adult_output_batches WHERE id=?")
            .bind(batch_id.to_string())
            .fetch_optional(self.repo.pool())
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let rows = sqlx::query("SELECT media_item_id,preview_id,state,result_json,error_code FROM adult_output_batch_items WHERE batch_id=? ORDER BY ordinal")
            .bind(batch_id.to_string()).fetch_all(self.repo.pool()).await?;
        let items = rows
            .into_iter()
            .map(|row| {
                let media: String = row.try_get("media_item_id")?;
                let preview: String = row.try_get("preview_id")?;
                let result: Option<String> = row.try_get("result_json")?;
                Ok(AdultBatchItemResult {
                    media_item_id: Uuid::parse_str(&media)
                        .map_err(|_| RepositoryError::CorruptData)?,
                    preview_id: Uuid::parse_str(&preview)
                        .map_err(|_| RepositoryError::CorruptData)?,
                    state: row.try_get("state")?,
                    error_code: row.try_get("error_code")?,
                    result: result
                        .map(|value| {
                            serde_json::from_str(&value).map_err(|_| RepositoryError::CorruptData)
                        })
                        .transpose()?,
                })
            })
            .collect::<Result<Vec<_>, RepositoryError>>()?;
        Ok(AdultBatchResult {
            schema_version: 1,
            batch_id,
            state,
            items,
        })
    }
    pub async fn execute(&self, batch_id: Uuid) -> Result<AdultBatchResult, RepositoryError> {
        self.output.ensure_access()?;
        if !self.output.is_adult_channel() {
            return Err(RepositoryError::InvalidInput);
        }
        let cancellation = CancellationToken::new();
        {
            let mut active = self.active.lock().await;
            if active.contains_key(&batch_id) {
                return Err(RepositoryError::Conflict);
            }
            active.insert(batch_id, cancellation.clone());
        }
        let result = self.execute_inner(batch_id, &cancellation).await;
        self.active.lock().await.remove(&batch_id);
        if result.is_ok() {
            self.output.ensure_access()?;
        }
        result
    }
    async fn execute_inner(
        &self,
        batch_id: Uuid,
        cancel: &CancellationToken,
    ) -> Result<AdultBatchResult, RepositoryError> {
        let changed = sqlx::query("UPDATE adult_output_batches SET state='running',updated_at=? WHERE id=? AND state IN ('planned','interrupted')")
            .bind(Utc::now().timestamp_millis()).bind(batch_id.to_string()).execute(self.repo.pool()).await?;
        if changed.rows_affected() != 1 {
            let result = self.status_inner(batch_id).await?;
            return if result.state == "succeeded" {
                for item in &result.items {
                    self.output.execute(item.preview_id).await?;
                }
                Ok(result)
            } else {
                Err(RepositoryError::Conflict)
            };
        }
        sqlx::query("UPDATE scrape_tasks SET state='running',updated_at=MAX(updated_at,?) WHERE id=? AND state='waiting_for_user'")
            .bind(Utc::now().timestamp_millis()).bind(batch_id.to_string()).execute(self.repo.pool()).await?;
        let initial = self.status_inner(batch_id).await?;
        for item in initial.items {
            if item.state == "succeeded" {
                continue;
            }
            if cancel.is_cancelled() || self.output.ensure_access().is_err() {
                self.cancel_inner(batch_id, cancel).await?;
                break;
            }
            let acquired = sqlx::query("UPDATE adult_output_batch_items SET state='running' WHERE batch_id=? AND preview_id=? AND state IN ('planned','interrupted') AND EXISTS(SELECT 1 FROM adult_output_batches b WHERE b.id=? AND b.state='running')")
                .bind(batch_id.to_string()).bind(item.preview_id.to_string()).bind(batch_id.to_string()).execute(self.repo.pool()).await?;
            if acquired.rows_affected() == 0 {
                continue;
            }
            let result = self
                .output
                .execute_cancellable(item.preview_id, Some(cancel))
                .await;
            let (state, json, error) = match result {
                Ok(result) => (
                    "succeeded",
                    Some(
                        serde_json::to_string(&result)
                            .map_err(|_| RepositoryError::InvalidInput)?,
                    ),
                    None,
                ),
                Err(error) => {
                    let cancelled = cancel.is_cancelled() || self.output.ensure_access().is_err();
                    (
                        if cancelled { "cancelled" } else { "failed" },
                        None,
                        Some(error_code(&error)),
                    )
                }
            };
            sqlx::query("UPDATE adult_output_batch_items SET state=?,result_json=?,error_code=? WHERE batch_id=? AND preview_id=?")
                .bind(state).bind(json).bind(error).bind(batch_id.to_string()).bind(item.preview_id.to_string()).execute(self.repo.pool()).await?;
        }
        let current = self.status_inner(batch_id).await?;
        if current.state == "running" {
            let state = if current.items.iter().any(|item| item.state == "failed") {
                "failed"
            } else if current.items.iter().any(|item| item.state == "cancelled") {
                "cancelled"
            } else {
                "succeeded"
            };
            let now = Utc::now().timestamp_millis();
            let mut tx = self.repo.pool().begin().await?;
            sqlx::query("UPDATE adult_output_batches SET state=?,updated_at=? WHERE id=? AND state='running'")
                .bind(state).bind(now).bind(batch_id.to_string()).execute(&mut *tx).await?;
            sqlx::query("UPDATE scrape_tasks SET state=?,updated_at=MAX(updated_at,?) WHERE id=? AND state='running'")
                .bind(state).bind(now).bind(batch_id.to_string()).execute(&mut *tx).await?;
            tx.commit().await?;
        }
        self.status_inner(batch_id).await
    }
    pub async fn cancel(&self, batch_id: Uuid) -> Result<AdultBatchResult, RepositoryError> {
        self.output.ensure_access()?;
        if !self.output.is_adult_channel() {
            return Err(RepositoryError::InvalidInput);
        }
        let cancellation = self
            .active
            .lock()
            .await
            .get(&batch_id)
            .cloned()
            .unwrap_or_default();
        self.cancel_inner(batch_id, &cancellation).await?;
        let result = self.status_inner(batch_id).await?;
        self.output.ensure_access()?;
        Ok(result)
    }
    async fn cancel_inner(
        &self,
        batch_id: Uuid,
        cancellation: &CancellationToken,
    ) -> Result<(), RepositoryError> {
        let now = Utc::now().timestamp_millis();
        let mut tx = self.repo.pool().begin().await?;
        let task_id: String =
            sqlx::query_scalar("SELECT task_id FROM adult_output_batches WHERE id=?")
                .bind(batch_id.to_string())
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(RepositoryError::NotFound)?;
        cancellation.cancel();
        sqlx::query("UPDATE adult_output_batches SET state='cancelled',updated_at=? WHERE id=? AND state IN ('planned','running','interrupted')")
            .bind(now).bind(batch_id.to_string()).execute(&mut *tx).await?;
        sqlx::query("UPDATE scrape_tasks SET state='cancelled',updated_at=MAX(updated_at,?) WHERE id=? AND state IN ('waiting_for_user','running','interrupted') AND EXISTS(SELECT 1 FROM adult_output_batches b WHERE b.id=? AND b.task_id=scrape_tasks.id)")
            .bind(now).bind(task_id).bind(batch_id.to_string()).execute(&mut *tx).await?;
        sqlx::query("UPDATE adult_output_batch_items SET state='cancelled',error_code='cancelled' WHERE batch_id=? AND state IN ('planned','interrupted')")
            .bind(batch_id.to_string()).execute(&mut *tx).await?;
        tx.commit().await?;
        // Discard unstarted plans to release platform source leases.
        let result = self.status_inner(batch_id).await?;
        for item in result.items.iter().filter(|item| item.state == "cancelled") {
            let _ = self.output.discard(item.preview_id).await;
        }
        Ok(())
    }
}
fn error_code(error: &RepositoryError) -> String {
    match error {
        RepositoryError::Provider(failure) => failure.code(),
        RepositoryError::NotFound => "not_found",
        RepositoryError::Conflict => "conflict",
        RepositoryError::ProtectedMetadata => "protected_metadata",
        RepositoryError::InvalidInput => "invalid_input",
        RepositoryError::Database => "database_error",
        RepositoryError::Migration => "migration_error",
        RepositoryError::CorruptData => "corrupt_data",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media_output::AdultOutputAccess;
    use mediaatlas_domain::{LibraryRoot, MediaFile, MediaItem, MediaKind, PathStyle};

    async fn library() -> (
        tempfile::TempDir,
        LibraryRepository,
        AdultBatchService,
        Vec<Uuid>,
        AdultOutputAccess,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let media = temp.path().join("synthetic-adult-fixtures");
        std::fs::create_dir(&media).unwrap();
        let repo = LibraryRepository::open(&temp.path().join("batch.sqlite"))
            .await
            .unwrap();
        let now = Utc::now();
        let root_id = Uuid::new_v4();
        repo.save_root(&LibraryRoot {
            id: root_id,
            name: "Synthetic adult fixtures".into(),
            path: media.to_string_lossy().into_owned(),
            path_style: if cfg!(windows) {
                PathStyle::Windows
            } else {
                PathStyle::Posix
            },
            kind: Some(MediaKind::AdultVideo),
            enabled: true,
            created_at: now,
            updated_at: now,
        })
        .await
        .unwrap();
        let mut ids = Vec::new();
        for index in 0..2 {
            let id = Uuid::new_v4();
            ids.push(id);
            let path = format!("synthetic-{index}.mkv");
            std::fs::write(media.join(&path), b"synthetic untouched media fixture").unwrap();
            repo.save_media(&MediaItem {
                id,
                kind: MediaKind::AdultVideo,
                title: format!("Synthetic neutral metadata {index}"),
                original_title: None,
                year: None,
                created_at: now,
                updated_at: now,
            })
            .await
            .unwrap();
            repo.save_file(&MediaFile {
                id: Uuid::new_v4(),
                media_item_id: id,
                root_id,
                relative_path: path,
                size_bytes: 33,
                modified_at: None,
                part: None,
            })
            .await
            .unwrap();
        }
        let access = AdultOutputAccess::new();
        let output = MediaOutputService::new_adult(
            repo.clone(),
            temp.path().join("adult-cache"),
            access.clone(),
        );
        let service = AdultBatchService::new(repo.clone(), output);
        (temp, repo, service, ids, access)
    }
    #[tokio::test]
    async fn adult_batch_requires_explicit_execute_records_per_item_and_preserves_video() {
        let (temp, repo, service, ids, _) = library().await;
        let preview = service.preview(ids).await.unwrap();
        assert_eq!(preview.items.len(), 2);
        let media = temp.path().join("synthetic-adult-fixtures");
        assert!(!media.join("synthetic-0.nfo").exists());
        let input: String =
            sqlx::query_scalar("SELECT input_snapshot FROM scrape_tasks WHERE id=?")
                .bind(preview.batch_id.to_string())
                .fetch_one(repo.pool())
                .await
                .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&input).unwrap()["adult"],
            true
        );
        assert!(
            sqlx::query("UPDATE adult_output_batch_items SET preview_id=? WHERE batch_id=?")
                .bind(Uuid::new_v4().to_string())
                .bind(preview.batch_id.to_string())
                .execute(repo.pool())
                .await
                .is_err()
        );
        let result = service.execute(preview.batch_id).await.unwrap();
        assert_eq!(result.state, "succeeded");
        assert!(result
            .items
            .iter()
            .all(|item| item.state == "succeeded" && item.result.as_ref().unwrap().written == 1));
        for index in 0..2 {
            assert_eq!(
                std::fs::read(media.join(format!("synthetic-{index}.mkv"))).unwrap(),
                b"synthetic untouched media fixture"
            );
        }
        assert_eq!(
            service.execute(preview.batch_id).await.unwrap().state,
            "succeeded"
        );
        assert_eq!(
            service.status(preview.batch_id).await.unwrap().items.len(),
            2
        );
        std::fs::write(
            media.join("synthetic-0.nfo"),
            "<movie><title>Manual edit</title></movie>",
        )
        .unwrap();
        assert_eq!(
            service.execute(preview.batch_id).await.unwrap_err(),
            RepositoryError::Conflict
        );
    }
    #[tokio::test]
    async fn adult_batch_cancel_prevents_all_unstarted_writes_and_cannot_be_replayed() {
        let (temp, _, service, ids, _) = library().await;
        assert_eq!(
            service.preview(vec![ids[0], ids[0]]).await.unwrap_err(),
            RepositoryError::InvalidInput
        );
        let preview = service.preview(ids).await.unwrap();
        let cancelled = service.cancel(preview.batch_id).await.unwrap();
        assert_eq!(cancelled.state, "cancelled");
        assert!(cancelled.items.iter().all(|item| item.state == "cancelled"));
        assert_eq!(
            service.execute(preview.batch_id).await.unwrap_err(),
            RepositoryError::Conflict
        );
        for index in 0..2 {
            assert!(!temp
                .path()
                .join(format!("synthetic-adult-fixtures/synthetic-{index}.nfo"))
                .exists());
        }
    }
    #[tokio::test]
    async fn adult_batch_isolates_file_conflicts_and_revoked_grants_fail_closed() {
        let (temp, _, service, ids, access) = library().await;
        let preview = service.preview(ids.clone()).await.unwrap();
        let first = temp.path().join("synthetic-adult-fixtures/synthetic-0.nfo");
        std::fs::write(
            &first,
            "<movie><title>Intervening manual file</title></movie>",
        )
        .unwrap();
        let result = service.execute(preview.batch_id).await.unwrap();
        assert_eq!(result.state, "failed");
        assert_eq!(result.items[0].state, "failed");
        assert_eq!(result.items[0].error_code.as_deref(), Some("conflict"));
        assert_eq!(result.items[1].state, "succeeded");
        assert!(std::fs::read_to_string(first)
            .unwrap()
            .contains("Intervening manual file"));
        let pending = service.preview(ids).await.unwrap();
        access.revoke();
        assert_eq!(
            service.execute(pending.batch_id).await.unwrap_err(),
            RepositoryError::Provider(mediaatlas_domain::ProviderFailure::Cancelled)
        );
        assert_eq!(
            service.status(pending.batch_id).await.unwrap_err(),
            RepositoryError::Provider(mediaatlas_domain::ProviderFailure::Cancelled)
        );
    }
    #[tokio::test]
    async fn batch_persistence_failure_discards_all_previews_without_touching_media() {
        let (temp, repo, service, ids, _) = library().await;
        sqlx::query("CREATE TRIGGER reject_synthetic_batch BEFORE INSERT ON adult_output_batches BEGIN SELECT RAISE(FAIL,'synthetic persistence failure'); END;")
            .execute(repo.pool()).await.unwrap();
        assert!(service.preview(ids).await.is_err());
        let active: i64 =
            sqlx::query_scalar("SELECT count(*) FROM file_operations WHERE state<>'interrupted'")
                .fetch_one(repo.pool())
                .await
                .unwrap();
        assert_eq!(active, 0);
        let batch_count: i64 = sqlx::query_scalar("SELECT count(*) FROM adult_output_batches")
            .fetch_one(repo.pool())
            .await
            .unwrap();
        assert_eq!(batch_count, 0);
        for index in 0..2 {
            assert!(!temp
                .path()
                .join(format!("synthetic-adult-fixtures/synthetic-{index}.nfo"))
                .exists());
        }
    }
    #[tokio::test]
    async fn cancel_rejects_unknown_and_ordinary_task_ids_without_cross_channel_mutation() {
        let (_temp, repo, service, _, _) = library().await;
        let ordinary = Uuid::new_v4();
        let now = Utc::now().timestamp_millis();
        sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES (?,'scan_library','running','{}',?,?)")
            .bind(ordinary.to_string()).bind(now).bind(now).execute(repo.pool()).await.unwrap();
        for id in [ordinary, Uuid::new_v4()] {
            assert_eq!(
                service.cancel(id).await.unwrap_err(),
                RepositoryError::NotFound
            );
        }
        let state: String = sqlx::query_scalar("SELECT state FROM scrape_tasks WHERE id=?")
            .bind(ordinary.to_string())
            .fetch_one(repo.pool())
            .await
            .unwrap();
        assert_eq!(state, "running");
        let cancelled: i64 =
            sqlx::query_scalar("SELECT count(*) FROM scrape_tasks WHERE state='cancelled'")
                .fetch_one(repo.pool())
                .await
                .unwrap();
        assert_eq!(cancelled, 0);
    }
    #[tokio::test]
    async fn separate_ipc_service_instances_cancel_an_inflight_item_before_publication() {
        let (temp, repo, service, ids, access) = library().await;
        let preview = service.preview(ids).await.unwrap();
        let other = AdultBatchService::new(
            repo.clone(),
            MediaOutputService::new_adult(repo.clone(), temp.path().join("adult-cache"), access),
        );
        let held = service.output.lock_for_test().await;
        let executing = service.clone();
        let batch_id = preview.batch_id;
        let worker = tokio::spawn(async move { executing.execute(batch_id).await });
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let state: String = sqlx::query_scalar(
                    "SELECT state FROM adult_output_batch_items WHERE batch_id=? AND ordinal=0",
                )
                .bind(batch_id.to_string())
                .fetch_one(repo.pool())
                .await
                .unwrap();
                if state == "running" {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(other.cancel(batch_id).await.unwrap().state, "cancelled");
        drop(held);
        let finished = worker.await.unwrap().unwrap();
        assert_eq!(finished.state, "cancelled");
        assert!(finished.items.iter().all(|item| item.state == "cancelled"));
        for index in 0..2 {
            assert!(!temp
                .path()
                .join(format!("synthetic-adult-fixtures/synthetic-{index}.nfo"))
                .exists());
        }
    }
    #[tokio::test]
    async fn batch_recovery_marks_interrupted_without_replaying_output() {
        let (temp, repo, service, ids, _) = library().await;
        let preview = service.preview(ids).await.unwrap();
        sqlx::query("UPDATE adult_output_batches SET state='running' WHERE id=?")
            .bind(preview.batch_id.to_string())
            .execute(repo.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE adult_output_batch_items SET state='running' WHERE batch_id=?")
            .bind(preview.batch_id.to_string())
            .execute(repo.pool())
            .await
            .unwrap();
        service.initialize().await.unwrap();
        assert_eq!(
            service.status(preview.batch_id).await.unwrap().state,
            "interrupted"
        );
        assert!(!temp
            .path()
            .join("synthetic-adult-fixtures/synthetic-0.nfo")
            .exists());
        assert_eq!(
            service.execute(preview.batch_id).await.unwrap().state,
            "succeeded"
        );
    }
}
