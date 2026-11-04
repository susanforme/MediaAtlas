//! Persistent local scan workflow. Automatic refresh indexes and enriches only
//! application data; manual scan previews remain available for isolated workflows.

mod automatic;
mod importer;

use chrono::{DateTime, Utc};
use mediaatlas_application::library::{
    ConfirmInput, ConfirmResult, PreviewInput, PreviewPageView, ScanCandidateView, ScanTaskView,
};
use mediaatlas_domain::{LibraryRoot, MediaKind, TaskState};
use mediaatlas_media_scanner::{run_scan, FileStamp, ScanOptions, ScanStats, ScannedFile};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqliteRow, Row};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use crate::library::{LibraryRepository, RepositoryError};

type AdultAutomatic = (
    crate::adult_scraper::AdultScraperService,
    crate::media_output::AdultOutputAccess,
);

#[derive(Clone)]
pub struct ScanService {
    repo: LibraryRepository,
    cancellations: Arc<Mutex<HashMap<Uuid, Arc<AtomicBool>>>>,
    gate: Arc<tokio::sync::Mutex<()>>,
    events: broadcast::Sender<ScanTaskView>,
    scraper: Option<crate::scraper::ScraperService>,
    network_cancellations: Arc<Mutex<HashMap<Uuid, crate::network::CancellationToken>>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct RootSnapshot {
    root: LibraryRoot,
    exclude_patterns: Vec<String>,
    #[serde(default)]
    metadata_providers: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize)]
struct PreviewCursor {
    task_id: Uuid,
    include_adult: bool,
    after: Uuid,
}

enum WorkerMessage {
    Entry(Box<ScannedFile>),
    Progress(ScanStats),
}

impl ScanService {
    pub fn new(repo: LibraryRepository) -> Self {
        let (events, _) = broadcast::channel(128);
        Self {
            repo,
            cancellations: Arc::new(Mutex::new(HashMap::new())),
            gate: Arc::new(tokio::sync::Mutex::new(())),
            events,
            scraper: None,
            network_cancellations: Arc::default(),
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ScanTaskView> {
        self.events.subscribe()
    }

    pub fn with_scraper(mut self, scraper: crate::scraper::ScraperService) -> Self {
        self.scraper = Some(scraper);
        self
    }

    /// A user-requested library refresh indexes local files and enriches metadata
    /// in the application database. It never creates media sidecars or moves files.
    pub async fn start_automatic(&self, root_id: Uuid) -> Result<ScanTaskView, RepositoryError> {
        self.start_inner(root_id, None, true, None).await
    }

    /// Adult refresh uses a revocable grant, isolated adapters and cache. Merely
    /// passing an adult Root to the ordinary entry point never grants access.
    pub async fn start_automatic_adult(
        &self,
        root_id: Uuid,
        scraper: crate::adult_scraper::AdultScraperService,
        access: crate::media_output::AdultOutputAccess,
    ) -> Result<ScanTaskView, RepositoryError> {
        access.ensure()?;
        self.start_inner(
            root_id,
            None,
            true,
            Some((scraper.with_access(access.clone()), access)),
        )
        .await
    }

    pub async fn start(&self, root_id: Uuid) -> Result<ScanTaskView, RepositoryError> {
        self.start_inner(root_id, None, false, None).await
    }

    /// Watcher refresh pipeline. Revalidate its exact live Root generation and
    /// opt-in under the SQLite writer reservation before creating any task.
    pub async fn start_watched(
        &self,
        expected: &LibraryRoot,
    ) -> Result<ScanTaskView, RepositoryError> {
        self.start_inner(expected.id, Some(expected), self.scraper.is_some(), None)
            .await
    }

    async fn start_inner(
        &self,
        root_id: Uuid,
        expected: Option<&LibraryRoot>,
        automatic: bool,
        adult: Option<AdultAutomatic>,
    ) -> Result<ScanTaskView, RepositoryError> {
        let _gate = self.gate.lock().await;
        if let Some((_, access)) = &adult {
            access.ensure()?;
        }
        let mut tx = self.repo.pool().begin().await?;
        // Acquire the writer reservation before the Root/settings snapshot. A
        // reclassification or path change cannot race the final task insertion.
        sqlx::query("UPDATE library_roots SET updated_at=updated_at WHERE id=?")
            .bind(root_id.to_string())
            .execute(&mut *tx)
            .await?;
        let snapshot = snapshot_tx(&mut tx, root_id).await?;
        if !snapshot.root.enabled {
            return Err(RepositoryError::InvalidInput);
        }
        if automatic {
            let isolated: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM media_files f JOIN media_items m ON m.id=f.media_item_id WHERE f.root_id=? AND (m.adult_isolated=1 OR m.kind='adult_video'))")
                .bind(root_id.to_string()).fetch_one(&mut *tx).await?;
            if (snapshot.root.kind == Some(MediaKind::AdultVideo)) != adult.is_some()
                || (adult.is_none() && isolated)
            {
                return Err(RepositoryError::InvalidInput);
            }
        }
        if let Some(expected) = expected {
            if &snapshot.root != expected
                || snapshot.root.kind.is_none()
                || snapshot.root.kind == Some(MediaKind::AdultVideo)
            {
                return Err(RepositoryError::Conflict);
            }
            let opted_in: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM watcher_roots WHERE root_id=? AND opted_in=1)",
            )
            .bind(root_id.to_string())
            .fetch_one(&mut *tx)
            .await?;
            let isolated:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM media_files f JOIN media_items m ON m.id=f.media_item_id WHERE f.root_id=? AND (m.adult_isolated=1 OR m.kind='adult_video'))")
                .bind(root_id.to_string()).fetch_one(&mut *tx).await?;
            if !opted_in || isolated {
                return Err(RepositoryError::Conflict);
            }
        }
        let active: i64 = sqlx::query_scalar("SELECT count(*) FROM scan_sessions s JOIN scrape_tasks t ON t.id=s.task_id WHERE s.root_id=? AND (t.state IN ('queued','running') OR (? AND t.state='waiting_for_user'))")
            .bind(root_id.to_string()).bind(expected.is_some() && !automatic).fetch_one(&mut *tx).await?;
        if active > 0 {
            return Err(RepositoryError::Conflict);
        }
        if automatic {
            // Replace stale discovery/review sessions. Their candidates and
            // metadata results remain history, not an extra approval gate.
            sqlx::query("UPDATE scrape_tasks SET state='cancelled',updated_at=? WHERE state='waiting_for_user' AND id IN (SELECT task_id FROM scan_sessions WHERE root_id=?)")
                .bind(Utc::now().timestamp_millis()).bind(root_id.to_string()).execute(&mut *tx).await?;
        }
        let id = Uuid::new_v4();
        let now = Utc::now().timestamp_millis();
        sqlx::query("INSERT INTO scrape_tasks (id,kind,state,input_snapshot,created_at,updated_at) VALUES (?,'scan_library','queued',?,?,?)")
            .bind(id.to_string()).bind(serde_json::json!({"root_id":root_id,"schema_version":1}).to_string()).bind(now).bind(now).execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO scan_sessions (task_id,root_id,root_snapshot_json,mode) VALUES (?,?,?,?)",
        )
        .bind(id.to_string())
        .bind(root_id.to_string())
        .bind(serde_json::to_string(&snapshot).map_err(|_| RepositoryError::InvalidInput)?)
        .bind(if automatic { "automatic" } else { "manual" })
        .execute(&mut *tx)
        .await?;
        if let Some((_, access)) = &adult {
            access.ensure()?;
        }
        tx.commit().await?;
        let cancellation = Arc::new(AtomicBool::new(false));
        self.cancellations
            .lock()
            .map_err(|_| RepositoryError::Database)?
            .insert(id, Arc::clone(&cancellation));
        let network_cancel = crate::network::CancellationToken::new();
        self.network_cancellations
            .lock()
            .map_err(|_| RepositoryError::Database)?
            .insert(id, network_cancel.clone());
        self.emit(id).await;
        let service = self.clone();
        tokio::spawn(async move {
            let grant_monitor = adult.as_ref().map(|(_, access)| {
                let access = access.clone();
                let flag = cancellation.clone();
                let network = network_cancel.clone();
                tokio::spawn(async move {
                    loop {
                        if access.ensure().is_err() {
                            flag.store(true, Ordering::Release);
                            network.cancel();
                            break;
                        }
                        tokio::select! {
                            _ = network.cancelled() => break,
                            _ = tokio::time::sleep(Duration::from_millis(50)) => {}
                        }
                    }
                })
            });
            if let Err(error) = service
                .run(id, snapshot, cancellation, automatic, network_cancel, adult)
                .await
            {
                let _ = service
                    .finish(
                        id,
                        if matches!(
                            error,
                            RepositoryError::Provider(
                                mediaatlas_domain::ProviderFailure::Cancelled
                            )
                        ) {
                            "cancelled"
                        } else {
                            "failed"
                        },
                        if matches!(
                            error,
                            RepositoryError::Provider(
                                mediaatlas_domain::ProviderFailure::Cancelled
                            )
                        ) {
                            None
                        } else {
                            Some("scan_failed")
                        },
                        None,
                    )
                    .await;
            }
            if let Some(monitor) = grant_monitor {
                monitor.abort();
            }
            if let Ok(mut registry) = service.cancellations.lock() {
                registry.remove(&id);
            }
            if let Ok(mut registry) = service.network_cancellations.lock() {
                registry.remove(&id);
            }
        });
        self.get_task(id).await
    }

    pub async fn list_tasks(
        &self,
        include_adult: bool,
    ) -> Result<Vec<ScanTaskView>, RepositoryError> {
        let rows = sqlx::query("SELECT s.*,t.state,t.created_at,t.updated_at,(SELECT count(*) FROM library_refresh_items i WHERE i.task_id=s.task_id AND i.state='needs_review') actual_review_count,(SELECT count(*) FROM library_refresh_items i WHERE i.task_id=s.task_id AND i.state='needs_review' AND i.reason='provider_unconfigured') unconfigured_count FROM scan_sessions s JOIN scrape_tasks t ON t.id=s.task_id JOIN library_roots r ON r.id=s.root_id WHERE (? OR ((r.kind IS NULL OR r.kind!='adult_video') AND NOT EXISTS(SELECT 1 FROM scan_candidates c WHERE c.task_id=s.task_id AND c.kind='adult_video'))) ORDER BY t.created_at DESC,t.rowid DESC LIMIT 200")
            .bind(include_adult).fetch_all(self.repo.pool()).await?;
        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            result.push(task_from_row(&row)?);
        }
        Ok(result)
    }

    pub async fn preview(&self, input: &PreviewInput) -> Result<PreviewPageView, RepositoryError> {
        if !(1..=200).contains(&input.limit) {
            return Err(RepositoryError::InvalidInput);
        }
        let snapshot = self.saved_snapshot(input.task_id).await?;
        if snapshot.root.kind == Some(MediaKind::AdultVideo) && !input.include_adult {
            return Err(RepositoryError::NotFound);
        }
        let after = if let Some(cursor) = &input.cursor {
            let parsed: PreviewCursor =
                serde_json::from_str(cursor).map_err(|_| RepositoryError::InvalidInput)?;
            if parsed.task_id != input.task_id || parsed.include_adult != input.include_adult {
                return Err(RepositoryError::InvalidInput);
            }
            parsed.after.to_string()
        } else {
            String::new()
        };
        let rows = sqlx::query("SELECT * FROM scan_candidates WHERE task_id=? AND id>? AND (? OR kind!='adult_video') ORDER BY id LIMIT ?")
            .bind(input.task_id.to_string()).bind(after).bind(input.include_adult).bind(i64::from(input.limit)+1).fetch_all(self.repo.pool()).await?;
        let has_more = rows.len() > input.limit as usize;
        let mut items = Vec::new();
        for row in rows.iter().take(input.limit as usize) {
            items.push(candidate_from_row(row)?);
        }
        let next_cursor = if has_more {
            items
                .last()
                .map(|last| {
                    serde_json::to_string(&PreviewCursor {
                        task_id: input.task_id,
                        include_adult: input.include_adult,
                        after: last.id,
                    })
                    .map_err(|_| RepositoryError::CorruptData)
                })
                .transpose()?
        } else {
            None
        };
        Ok(PreviewPageView {
            task_id: input.task_id,
            items,
            next_cursor,
        })
    }

    pub async fn cancel(&self, task_id: Uuid) -> Result<ScanTaskView, RepositoryError> {
        let _gate = self.gate.lock().await;
        let task = self.get_task(task_id).await?;
        if matches!(
            task.state,
            TaskState::Queued | TaskState::Running | TaskState::WaitingForUser
        ) {
            if let Some(flag) = self
                .cancellations
                .lock()
                .map_err(|_| RepositoryError::Database)?
                .get(&task_id)
            {
                flag.store(true, Ordering::Release);
            }
            if let Some(token) = self
                .network_cancellations
                .lock()
                .map_err(|_| RepositoryError::Database)?
                .get(&task_id)
            {
                token.cancel();
            }
            sqlx::query("UPDATE scrape_tasks SET state='cancelled',updated_at=? WHERE id=? AND state IN ('queued','running','waiting_for_user')")
                .bind(Utc::now().timestamp_millis()).bind(task_id.to_string()).execute(self.repo.pool()).await?;
            self.emit(task_id).await;
        }
        self.get_task(task_id).await
    }

    pub async fn recover_interrupted(&self) -> Result<(), RepositoryError> {
        // Only the composition root calls this once before accepting any new work.
        sqlx::query("UPDATE scrape_tasks SET state='interrupted',updated_at=? WHERE kind='scan_library' AND state IN ('queued','running') AND id IN (SELECT task_id FROM scan_sessions)")
            .bind(Utc::now().timestamp_millis()).execute(self.repo.pool()).await?;
        sqlx::query("UPDATE library_refresh_items SET state='needs_review',reason='interrupted',updated_at=? WHERE state='pending' AND task_id IN (SELECT id FROM scrape_tasks WHERE state='interrupted')")
            .bind(Utc::now().timestamp_millis()).execute(self.repo.pool()).await?;
        sqlx::query("UPDATE scan_sessions SET review_count=(SELECT count(*) FROM library_refresh_items i WHERE i.task_id=scan_sessions.task_id AND i.state='needs_review') WHERE mode='automatic'")
            .execute(self.repo.pool()).await?;
        Ok(())
    }

    pub async fn confirm(&self, input: &ConfirmInput) -> Result<ConfirmResult, RepositoryError> {
        self.confirm_inner(input, false, None).await
    }

    async fn confirm_inner(
        &self,
        input: &ConfirmInput,
        automatic: bool,
        adult_access: Option<&crate::media_output::AdultOutputAccess>,
    ) -> Result<ConfirmResult, RepositoryError> {
        let _gate = self.gate.lock().await;
        if let Some(access) = adult_access {
            access.ensure()?;
        }
        if input.candidate_ids.is_empty()
            || input.candidate_ids.len() > 200
            || input.candidate_ids.iter().collect::<HashSet<_>>().len() != input.candidate_ids.len()
        {
            return Err(RepositoryError::InvalidInput);
        }
        let task = self.get_task(input.task_id).await?;
        if if automatic {
            task.state != TaskState::Running || task.mode != "automatic"
        } else {
            task.mode == "automatic"
                || !matches!(task.state, TaskState::WaitingForUser | TaskState::Succeeded)
        } {
            return Err(RepositoryError::Conflict);
        }
        let saved = self.saved_snapshot(input.task_id).await?;
        if automatic && (saved.root.kind == Some(MediaKind::AdultVideo)) != adult_access.is_some() {
            return Err(RepositoryError::InvalidInput);
        }
        let current = self.snapshot(saved.root.id).await?;
        if !same_config(&saved, &current) || !current.root.enabled {
            return Err(RepositoryError::Conflict);
        }
        let mut selected = Vec::new();
        for id in &input.candidate_ids {
            let row = sqlx::query("SELECT * FROM scan_candidates WHERE id=? AND task_id=?")
                .bind(id.to_string())
                .bind(input.task_id.to_string())
                .fetch_optional(self.repo.pool())
                .await?
                .ok_or(RepositoryError::NotFound)?;
            let view = candidate_from_row(&row)?;
            let file: ScannedFile = serde_json::from_str(&text(&row, "payload_json")?)
                .map_err(|_| RepositoryError::CorruptData)?;
            selected.push((view, file));
        }
        let root_path = saved.root.path.clone();
        let verify = selected
            .iter()
            .filter(|(view, _)| view.status == "pending")
            .map(|(_, file)| file.clone())
            .collect::<Vec<_>>();
        tokio::task::spawn_blocking(move || importer::validate_sources(&root_path, &verify))
            .await
            .map_err(|_| RepositoryError::Database)??;
        let mut tx = self.repo.pool().begin().await?;
        if let Some(access) = adult_access {
            access.ensure()?;
        }
        // Config is checked once more after acquiring the SQLite write transaction.
        let root_row =
            sqlx::query("SELECT path,path_style,kind,enabled FROM library_roots WHERE id=?")
                .bind(saved.root.id.to_string())
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(RepositoryError::NotFound)?;
        if text(&root_row, "path")? != saved.root.path
            || text(&root_row, "path_style")? != saved.root.path_style.as_str()
            || root_row
                .try_get::<Option<String>, _>("kind")
                .map_err(|_| RepositoryError::CorruptData)?
                != saved.root.kind.map(|k| k.as_str().to_owned())
            || !root_row
                .try_get::<bool, _>("enabled")
                .map_err(|_| RepositoryError::CorruptData)?
        {
            return Err(RepositoryError::Conflict);
        }
        let settings: Option<String> =
            sqlx::query_scalar("SELECT exclude_patterns_json FROM root_settings WHERE root_id=?")
                .bind(saved.root.id.to_string())
                .fetch_optional(&mut *tx)
                .await?;
        let exclusions: Vec<String> = serde_json::from_str(settings.as_deref().unwrap_or("[]"))
            .map_err(|_| RepositoryError::CorruptData)?;
        let providers: Option<Vec<String>> = sqlx::query_scalar::<_, Option<String>>("SELECT metadata_providers_json FROM root_settings WHERE root_id=?")
            .bind(saved.root.id.to_string()).fetch_optional(&mut *tx).await?.flatten()
            .map(|value| serde_json::from_str(&value).map_err(|_| RepositoryError::CorruptData)).transpose()?;
        if exclusions != saved.exclude_patterns || providers != saved.metadata_providers {
            return Err(RepositoryError::Conflict);
        }
        let current_state: String = sqlx::query_scalar("SELECT state FROM scrape_tasks WHERE id=?")
            .bind(input.task_id.to_string())
            .fetch_one(&mut *tx)
            .await?;
        if if automatic {
            current_state != "running"
        } else {
            !matches!(current_state.as_str(), "waiting_for_user" | "succeeded")
        } {
            return Err(RepositoryError::Conflict);
        }
        let mut result = ConfirmResult::default();
        for (view, file) in selected {
            if view.status != "pending" {
                result.unchanged += 1;
                continue;
            }
            let imported = importer::import_file(&mut tx, &saved.root, &file).await?;
            sqlx::query("UPDATE scan_candidates SET status='imported',existing_media_id=? WHERE id=? AND task_id=? AND status='pending'")
                .bind(imported.to_string()).bind(view.id.to_string()).bind(input.task_id.to_string()).execute(&mut *tx).await?;
            sqlx::query("INSERT INTO scan_source_fingerprints(root_id,normalized_relative_path,signature) VALUES(?,?,?) ON CONFLICT(root_id,normalized_relative_path) DO UPDATE SET signature=excluded.signature")
                .bind(saved.root.id.to_string()).bind(&file.normalized_relative_path).bind(&file.sidecar_fingerprint).execute(&mut *tx).await?;
            result.imported += 1;
            if view.needs_review {
                result.needs_review += 1;
            }
        }
        let remaining: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM scan_candidates WHERE task_id=? AND status='pending'",
        )
        .bind(input.task_id.to_string())
        .fetch_one(&mut *tx)
        .await?;
        sqlx::query("UPDATE scan_sessions SET imported_count=(SELECT count(*) FROM scan_candidates WHERE task_id=? AND status='imported') WHERE task_id=?")
            .bind(input.task_id.to_string()).bind(input.task_id.to_string()).execute(&mut *tx).await?;
        if !automatic {
            sqlx::query("UPDATE scrape_tasks SET state=?,updated_at=? WHERE id=? AND state IN ('waiting_for_user','succeeded')")
            .bind(if remaining==0 {"succeeded"} else {"waiting_for_user"}).bind(Utc::now().timestamp_millis()).bind(input.task_id.to_string()).execute(&mut *tx).await?;
        }
        if let Some(access) = adult_access {
            access.ensure()?;
        }
        tx.commit().await?;
        self.emit(input.task_id).await;
        Ok(result)
    }

    async fn snapshot(&self, root_id: Uuid) -> Result<RootSnapshot, RepositoryError> {
        let root = self
            .repo
            .get_root(root_id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let settings: Option<String> =
            sqlx::query_scalar("SELECT exclude_patterns_json FROM root_settings WHERE root_id=?")
                .bind(root_id.to_string())
                .fetch_optional(self.repo.pool())
                .await?;
        let exclude_patterns = serde_json::from_str(settings.as_deref().unwrap_or("[]"))
            .map_err(|_| RepositoryError::CorruptData)?;
        Ok(RootSnapshot {
            root,
            exclude_patterns,
            metadata_providers: crate::root_sources::load(self.repo.pool(), root_id).await?,
        })
    }

    async fn saved_snapshot(&self, task_id: Uuid) -> Result<RootSnapshot, RepositoryError> {
        let saved: Option<String> =
            sqlx::query_scalar("SELECT root_snapshot_json FROM scan_sessions WHERE task_id=?")
                .bind(task_id.to_string())
                .fetch_optional(self.repo.pool())
                .await?;
        serde_json::from_str(&saved.ok_or(RepositoryError::NotFound)?)
            .map_err(|_| RepositoryError::CorruptData)
    }

    async fn get_task(&self, id: Uuid) -> Result<ScanTaskView, RepositoryError> {
        let row=sqlx::query("SELECT s.*,t.state,t.created_at,t.updated_at,(SELECT count(*) FROM library_refresh_items i WHERE i.task_id=s.task_id AND i.state='needs_review') actual_review_count,(SELECT count(*) FROM library_refresh_items i WHERE i.task_id=s.task_id AND i.state='needs_review' AND i.reason='provider_unconfigured') unconfigured_count FROM scan_sessions s JOIN scrape_tasks t ON t.id=s.task_id WHERE s.task_id=?").bind(id.to_string()).fetch_optional(self.repo.pool()).await?.ok_or(RepositoryError::NotFound)?;
        task_from_row(&row)
    }

    async fn emit(&self, id: Uuid) {
        if let Ok(view) = self.get_task(id).await {
            let _ = self.events.send(view);
        }
    }

    async fn run(
        &self,
        id: Uuid,
        snapshot: RootSnapshot,
        cancel: Arc<AtomicBool>,
        automatic: bool,
        network_cancel: crate::network::CancellationToken,
        adult: Option<AdultAutomatic>,
    ) -> Result<(), RepositoryError> {
        sqlx::query(
            "UPDATE scrape_tasks SET state='running',updated_at=? WHERE id=? AND state='queued'",
        )
        .bind(Utc::now().timestamp_millis())
        .bind(id.to_string())
        .execute(self.repo.pool())
        .await?;
        if self.get_task(id).await?.state != TaskState::Running {
            return Ok(());
        }
        self.emit(id).await;
        let rows=sqlx::query("SELECT normalized_relative_path,size_bytes,modified_at FROM media_files WHERE root_id=?").bind(snapshot.root.id.to_string()).fetch_all(self.repo.pool()).await?;
        let mut known_files = HashMap::new();
        for row in rows {
            known_files.insert(
                text(&row, "normalized_relative_path")?,
                FileStamp {
                    size_bytes: nonnegative(&row, "size_bytes")?,
                    modified_at: optional_time(&row, "modified_at")?,
                },
            );
        }
        let signature_rows=sqlx::query("SELECT normalized_relative_path,signature FROM scan_source_fingerprints WHERE root_id=?").bind(snapshot.root.id.to_string()).fetch_all(self.repo.pool()).await?;
        let signatures: HashMap<String, String> = signature_rows
            .iter()
            .map(|row| {
                Ok((
                    text(row, "normalized_relative_path")?,
                    text(row, "signature")?,
                ))
            })
            .collect::<Result<_, RepositoryError>>()?;
        let options = ScanOptions {
            kind: snapshot.root.kind,
            path_style: snapshot.root.path_style,
            exclude_patterns: snapshot.exclude_patterns.clone(),
            known_files,
        };
        let (sender, mut receiver) = mpsc::channel(128);
        let progress = sender.clone();
        let root_path = snapshot.root.path.clone();
        let worker_cancel = Arc::clone(&cancel);
        let worker = tokio::task::spawn_blocking(move || {
            let mut last = Instant::now();
            let mut visited = 0;
            run_scan(
                std::path::Path::new(&root_path),
                &options,
                &worker_cancel,
                |file| {
                    sender
                        .blocking_send(WorkerMessage::Entry(Box::new(file)))
                        .is_ok()
                },
                |stats| {
                    if stats.visited_files.saturating_sub(visited) >= 100
                        || last.elapsed() >= Duration::from_millis(200)
                    {
                        visited = stats.visited_files;
                        last = Instant::now();
                        let _ = progress.blocking_send(WorkerMessage::Progress(stats));
                    }
                },
            )
        });
        while let Some(message) = receiver.recv().await {
            if cancel.load(Ordering::Acquire) {
                receiver.close();
                break;
            }
            match message {
                WorkerMessage::Entry(mut file) => {
                    if file.unchanged
                        && signatures.get(&file.normalized_relative_path)
                            != Some(&file.sidecar_fingerprint)
                    {
                        file.unchanged = false;
                    }
                    let owner:Option<String>=sqlx::query_scalar("SELECT media_item_id FROM media_files WHERE root_id=? AND normalized_relative_path=?").bind(snapshot.root.id.to_string()).bind(&file.normalized_relative_path).fetch_optional(self.repo.pool()).await?;
                    sqlx::query("INSERT INTO scan_candidates(id,task_id,root_id,kind,relative_path,normalized_relative_path,payload_json,status,existing_media_id) VALUES(?,?,?,?,?,?,?,?,?)")
                        .bind(Uuid::new_v4().to_string()).bind(id.to_string()).bind(snapshot.root.id.to_string()).bind(file.parsed.kind.as_str()).bind(&file.relative_path).bind(&file.normalized_relative_path)
                        .bind(serde_json::to_string(&file).map_err(|_|RepositoryError::CorruptData)?).bind(if file.unchanged {"unchanged"} else {"pending"}).bind(owner).execute(self.repo.pool()).await?;
                }
                WorkerMessage::Progress(stats) => self.store_progress(id, &stats).await?,
            }
        }
        // Drop pending messages before joining a worker that might be blocked on send.
        drop(receiver);
        let result = worker.await.map_err(|_| RepositoryError::Database)?;
        match result {
            Ok(stats) => {
                self.store_progress(id, &stats).await?;
                if cancel.load(Ordering::Acquire) || stats.cancelled {
                    return self.finish(id, "cancelled", None, None).await;
                }
                if automatic {
                    return self
                        .complete_automatic(id, &snapshot, &cancel, network_cancel, adult)
                        .await;
                }
                let pending: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM scan_candidates WHERE task_id=? AND status='pending'",
                )
                .bind(id.to_string())
                .fetch_one(self.repo.pool())
                .await?;
                self.finish(
                    id,
                    if pending == 0 {
                        "succeeded"
                    } else {
                        "waiting_for_user"
                    },
                    None,
                    Some(snapshot.root.id),
                )
                .await
            }
            Err(error) => self.finish(id, "failed", Some(error.code()), None).await,
        }
    }

    async fn store_progress(&self, id: Uuid, stats: &ScanStats) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE scan_sessions SET visited_files=?,skipped_files=?,candidate_count=?,warnings=? WHERE task_id=?")
            .bind(stats.visited_files as i64).bind(stats.skipped_files as i64).bind(stats.candidate_count as i64).bind(stats.warnings as i64).bind(id.to_string()).execute(self.repo.pool()).await?;
        sqlx::query("UPDATE scrape_tasks SET updated_at=? WHERE id=? AND state='running'")
            .bind(Utc::now().timestamp_millis())
            .bind(id.to_string())
            .execute(self.repo.pool())
            .await?;
        self.emit(id).await;
        Ok(())
    }

    async fn finish(
        &self,
        id: Uuid,
        state: &str,
        error: Option<&str>,
        root_id: Option<Uuid>,
    ) -> Result<(), RepositoryError> {
        let mut tx = self.repo.pool().begin().await?;
        sqlx::query("UPDATE scrape_tasks SET state=?,updated_at=? WHERE id=? AND state IN ('queued','running')").bind(state).bind(Utc::now().timestamp_millis()).bind(id.to_string()).execute(&mut *tx).await?;
        sqlx::query("UPDATE scan_sessions SET error_code=? WHERE task_id=?")
            .bind(error)
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?;
        if matches!(state, "failed" | "cancelled") {
            sqlx::query("UPDATE library_refresh_items SET state='needs_review',reason=?,updated_at=? WHERE task_id=? AND state='pending'")
                .bind(if state == "cancelled" { "cancelled" } else { "interrupted" }).bind(Utc::now().timestamp_millis()).bind(id.to_string()).execute(&mut *tx).await?;
            sqlx::query("UPDATE scan_sessions SET review_count=(SELECT count(*) FROM library_refresh_items WHERE task_id=? AND state='needs_review') WHERE task_id=?")
                .bind(id.to_string()).bind(id.to_string()).execute(&mut *tx).await?;
        }
        if let Some(root_id) = root_id {
            sqlx::query("INSERT INTO root_settings(root_id,last_scan_at) VALUES(?,?) ON CONFLICT(root_id) DO UPDATE SET last_scan_at=excluded.last_scan_at").bind(root_id.to_string()).bind(Utc::now().timestamp_millis()).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        self.emit(id).await;
        Ok(())
    }
}

fn same_config(a: &RootSnapshot, b: &RootSnapshot) -> bool {
    a.root.path == b.root.path
        && a.root.path_style == b.root.path_style
        && a.root.kind == b.root.kind
        && a.exclude_patterns == b.exclude_patterns
        && a.metadata_providers == b.metadata_providers
}

async fn snapshot_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    root_id: Uuid,
) -> Result<RootSnapshot, RepositoryError> {
    let row = sqlx::query("SELECT * FROM library_roots WHERE id=?")
        .bind(root_id.to_string())
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(RepositoryError::NotFound)?;
    let kind: Option<String> = row.try_get("kind")?;
    let root = LibraryRoot {
        id: uuid(&row, "id")?,
        name: text(&row, "name")?,
        path: text(&row, "path")?,
        path_style: serde_json::from_value(serde_json::Value::String(text(&row, "path_style")?))
            .map_err(|_| RepositoryError::CorruptData)?,
        kind: kind
            .map(|kind| {
                serde_json::from_value(serde_json::Value::String(kind))
                    .map_err(|_| RepositoryError::CorruptData)
            })
            .transpose()?,
        enabled: row.try_get("enabled")?,
        created_at: time(&row, "created_at")?,
        updated_at: time(&row, "updated_at")?,
    };
    let settings: Option<String> =
        sqlx::query_scalar("SELECT exclude_patterns_json FROM root_settings WHERE root_id=?")
            .bind(root_id.to_string())
            .fetch_optional(&mut **tx)
            .await?;
    let exclude_patterns = serde_json::from_str(settings.as_deref().unwrap_or("[]"))
        .map_err(|_| RepositoryError::CorruptData)?;
    Ok(RootSnapshot {
        root,
        exclude_patterns,
        metadata_providers: sqlx::query_scalar::<_, Option<String>>("SELECT metadata_providers_json FROM root_settings WHERE root_id=?")
            .bind(root_id.to_string()).fetch_optional(&mut **tx).await?.flatten()
            .map(|value| serde_json::from_str(&value).map_err(|_| RepositoryError::CorruptData)).transpose()?,
    })
}

fn task_from_row(row: &SqliteRow) -> Result<ScanTaskView, RepositoryError> {
    Ok(ScanTaskView {
        id: uuid(row, "task_id")?,
        root_id: uuid(row, "root_id")?,
        state: serde_json::from_value(serde_json::Value::String(text(row, "state")?))
            .map_err(|_| RepositoryError::CorruptData)?,
        visited_files: nonnegative(row, "visited_files")?,
        skipped_files: nonnegative(row, "skipped_files")?,
        candidate_count: nonnegative(row, "candidate_count")?,
        imported_count: nonnegative(row, "imported_count")?,
        warnings: nonnegative(row, "warnings")?,
        error_code: row
            .try_get("error_code")
            .map_err(|_| RepositoryError::CorruptData)?,
        mode: text(row, "mode")?,
        phase: text(row, "phase")?,
        matched_count: nonnegative(row, "matched_count")?,
        review_count: nonnegative(row, "actual_review_count")?,
        unconfigured_count: nonnegative(row, "unconfigured_count")?,
        metadata_failed_count: nonnegative(row, "metadata_failed_count")?,
        created_at: time(row, "created_at")?,
        updated_at: time(row, "updated_at")?,
    })
}

fn candidate_from_row(row: &SqliteRow) -> Result<ScanCandidateView, RepositoryError> {
    let file: ScannedFile = serde_json::from_str(&text(row, "payload_json")?)
        .map_err(|_| RepositoryError::CorruptData)?;
    let existing: Option<String> = row
        .try_get("existing_media_id")
        .map_err(|_| RepositoryError::CorruptData)?;
    Ok(ScanCandidateView {
        id: uuid(row, "id")?,
        root_id: uuid(row, "root_id")?,
        relative_path: file.relative_path,
        size_bytes: file.size_bytes,
        modified_at: file.modified_at,
        kind: file.parsed.kind,
        title: file.parsed.title,
        year: file.parsed.year,
        part: file.parsed.part,
        series_title: file.parsed.series_title,
        season_number: file.parsed.season_number,
        episode_number: file.parsed.episode_number,
        absolute_number: file.parsed.absolute_number,
        catalog_number: file.parsed.catalog_number,
        needs_review: file.parsed.needs_review,
        warnings: file.parsed.warnings,
        existing_media_id: existing
            .map(|value| Uuid::parse_str(&value).map_err(|_| RepositoryError::CorruptData))
            .transpose()?,
        status: text(row, "status")?,
    })
}

fn text(row: &SqliteRow, name: &str) -> Result<String, RepositoryError> {
    row.try_get(name).map_err(|_| RepositoryError::CorruptData)
}
fn uuid(row: &SqliteRow, name: &str) -> Result<Uuid, RepositoryError> {
    Uuid::parse_str(&text(row, name)?).map_err(|_| RepositoryError::CorruptData)
}
fn nonnegative(row: &SqliteRow, name: &str) -> Result<u64, RepositoryError> {
    u64::try_from(
        row.try_get::<i64, _>(name)
            .map_err(|_| RepositoryError::CorruptData)?,
    )
    .map_err(|_| RepositoryError::CorruptData)
}
fn time(row: &SqliteRow, name: &str) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::from_timestamp_millis(
        row.try_get(name)
            .map_err(|_| RepositoryError::CorruptData)?,
    )
    .ok_or(RepositoryError::CorruptData)
}
fn optional_time(row: &SqliteRow, name: &str) -> Result<Option<DateTime<Utc>>, RepositoryError> {
    row.try_get::<Option<i64>, _>(name)
        .map_err(|_| RepositoryError::CorruptData)?
        .map(|value| DateTime::from_timestamp_millis(value).ok_or(RepositoryError::CorruptData))
        .transpose()
}
