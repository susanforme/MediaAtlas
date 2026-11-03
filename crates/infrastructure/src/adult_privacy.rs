//! Durable exposure preferences and a revocable process-local adult grant.
//! This is a local privacy control, not an operating-system authentication claim.
use crate::{
    library::{LibraryRepository, RepositoryError},
    media_output::AdultOutputAccess,
    providers::ProviderService,
};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AdultSettings {
    pub entry_visible: bool,
    pub covers_visible: bool,
    pub remember_recent: bool,
    pub unlocked: bool,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AdultSettingsInput {
    pub entry_visible: bool,
    pub covers_visible: bool,
    pub remember_recent: bool,
}
#[derive(Clone)]
pub struct AdultPrivacyService {
    repo: LibraryRepository,
    providers: ProviderService,
    grant: Arc<Mutex<Option<AdultOutputAccess>>>,
    gate: Arc<Mutex<()>>,
}
impl AdultPrivacyService {
    pub fn new(repo: LibraryRepository, providers: ProviderService) -> Self {
        Self {
            repo,
            providers,
            grant: Arc::default(),
            gate: Arc::default(),
        }
    }
    pub async fn settings(&self) -> Result<AdultSettings, RepositoryError> {
        let row=sqlx::query("SELECT entry_visible,covers_visible,remember_recent FROM adult_privacy_settings WHERE id=1").fetch_one(self.repo.pool()).await?;
        let unlocked = self
            .grant
            .lock()
            .await
            .as_ref()
            .is_some_and(|grant| grant.ensure().is_ok());
        Ok(AdultSettings {
            entry_visible: row.try_get("entry_visible")?,
            covers_visible: row.try_get("covers_visible")?,
            remember_recent: row.try_get("remember_recent")?,
            unlocked,
        })
    }
    pub async fn save(&self, input: AdultSettingsInput) -> Result<AdultSettings, RepositoryError> {
        let _gate = self.gate.lock().await;
        let mut tx = self.repo.pool().begin().await?;
        sqlx::query("UPDATE adult_privacy_settings SET entry_visible=?,covers_visible=?,remember_recent=? WHERE id=1").bind(input.entry_visible).bind(input.covers_visible).bind(input.remember_recent).execute(&mut *tx).await?;
        if !input.remember_recent {
            sqlx::query("DELETE FROM adult_recent_visits")
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        if !input.entry_visible {
            self.lock_inner().await?;
        }
        self.settings().await
    }
    pub async fn unlock(&self) -> Result<AdultSettings, RepositoryError> {
        let _gate = self.gate.lock().await;
        if !self.settings().await?.entry_visible {
            return Err(RepositoryError::ProtectedMetadata);
        }
        let mut grant = self.grant.lock().await;
        if grant.as_ref().is_none_or(|grant| grant.ensure().is_err()) {
            *grant = Some(AdultOutputAccess::new());
        }
        drop(grant);
        self.settings().await
    }
    pub async fn lock(&self) -> Result<AdultSettings, RepositoryError> {
        let _gate = self.gate.lock().await;
        self.lock_inner().await?;
        self.settings().await
    }
    async fn lock_inner(&self) -> Result<(), RepositoryError> {
        if let Some(grant) = self.grant.lock().await.take() {
            grant.revoke();
        }
        self.providers.cancel_adult_requests().await?;
        crate::media_output::release_revoked_adult_leases().await;
        sqlx::query("INSERT OR IGNORE INTO adult_revoked_output_previews(preview_id) SELECT p.id FROM output_previews p JOIN file_operations o ON o.id=p.id WHERE json_extract(p.snapshot,'$.channel')='adult' AND o.state<>'succeeded'").execute(self.repo.pool()).await?;
        let ids=sqlx::query_scalar::<_,String>("SELECT t.id FROM scrape_tasks t JOIN adult_scrape_sessions a ON a.task_id=t.id WHERE t.state IN('queued','running','waiting_for_user')").fetch_all(self.repo.pool()).await?;
        for id in ids {
            let id = Uuid::parse_str(&id).map_err(|_| RepositoryError::CorruptData)?;
            self.providers.cancel(id).await?;
        }
        Ok(())
    }
    /// A short local confirmation may finish before a concurrent lock is
    /// acknowledged; long-running network/output operations use revocation instead.
    pub async fn confirmation_guard(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.gate.lock().await
    }
    pub async fn access(&self) -> Result<AdultOutputAccess, RepositoryError> {
        let grant = self
            .grant
            .lock()
            .await
            .clone()
            .ok_or(RepositoryError::ProtectedMetadata)?;
        grant.ensure()?;
        Ok(grant)
    }
    pub async fn require_covers(&self) -> Result<AdultOutputAccess, RepositoryError> {
        let access = self.access().await?;
        if !self.settings().await?.covers_visible {
            return Err(RepositoryError::ProtectedMetadata);
        }
        Ok(access)
    }
    pub async fn record_visit(&self, media_id: Uuid) -> Result<(), RepositoryError> {
        let _gate = self.gate.lock().await;
        let access = self.access().await?;
        if self.settings().await?.remember_recent {
            let mut tx = self.repo.pool().begin().await?;
            access.ensure()?;
            sqlx::query("INSERT INTO adult_recent_visits(media_item_id,visited_at) VALUES (?,?) ON CONFLICT(media_item_id) DO UPDATE SET visited_at=excluded.visited_at").bind(media_id.to_string()).bind(chrono::Utc::now().timestamp_millis()).execute(&mut *tx).await?;
            access.ensure()?;
            tx.commit().await?;
        }
        Ok(())
    }
}

impl AdultPrivacyService {
    pub async fn authorize_root(&self, root_id: Uuid) -> Result<(), RepositoryError> {
        let root = self
            .repo
            .get_root(root_id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        if root.kind == Some(mediaatlas_domain::MediaKind::AdultVideo) {
            self.access().await?;
        }
        Ok(())
    }
    pub async fn authorize_task(&self, task_id: Uuid) -> Result<(), RepositoryError> {
        let adult:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM scrape_tasks t WHERE t.id=? AND (COALESCE(json_extract(t.input_snapshot,'$.adult'),0)=1 OR EXISTS(SELECT 1 FROM scan_sessions s JOIN library_roots r ON r.id=s.root_id WHERE s.task_id=t.id AND r.kind='adult_video') OR EXISTS(SELECT 1 FROM scan_candidates c WHERE c.task_id=t.id AND c.kind='adult_video')))").bind(task_id.to_string()).fetch_one(self.repo.pool()).await?;
        if adult {
            self.access().await?;
        }
        Ok(())
    }
    pub async fn ensure_adult_media(&self, media_id: Uuid) -> Result<(), RepositoryError> {
        let access = self.access().await?;
        let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM media_items m WHERE m.id=? AND m.kind='adult_video' AND m.adult_isolated=1 AND EXISTS(SELECT 1 FROM media_files f JOIN library_roots r ON r.id=f.root_id WHERE f.media_item_id=m.id AND r.kind='adult_video' AND r.enabled=1) AND NOT EXISTS(SELECT 1 FROM media_files f JOIN library_roots r ON r.id=f.root_id WHERE f.media_item_id=m.id AND (r.kind IS NULL OR r.kind<>'adult_video')))").bind(media_id.to_string()).fetch_one(self.repo.pool()).await?;
        if !valid {
            return Err(RepositoryError::ProtectedMetadata);
        }
        access.ensure()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::NetworkEngine;
    async fn fixture() -> (tempfile::TempDir, LibraryRepository, AdultPrivacyService) {
        let dir = tempfile::tempdir().unwrap();
        let repo = LibraryRepository::open(&dir.path().join("synthetic.sqlite"))
            .await
            .unwrap();
        let providers = ProviderService::with_engine(repo.clone(), NetworkEngine::for_loopback(1))
            .await
            .unwrap();
        let privacy = AdultPrivacyService::new(repo.clone(), providers);
        (dir, repo, privacy)
    }
    #[tokio::test]
    async fn default_hidden_explicit_unlock_and_revoke_are_backend_authoritative() {
        let (_dir, _repo, privacy) = fixture().await;
        assert_eq!(
            privacy.settings().await.unwrap(),
            AdultSettings {
                entry_visible: false,
                covers_visible: false,
                remember_recent: false,
                unlocked: false
            }
        );
        assert!(privacy.unlock().await.is_err());
        assert!(privacy.access().await.is_err());
        privacy
            .save(AdultSettingsInput {
                entry_visible: true,
                covers_visible: false,
                remember_recent: false,
            })
            .await
            .unwrap();
        assert!(privacy.access().await.is_err());
        privacy.unlock().await.unwrap();
        let grant = privacy.access().await.unwrap();
        assert!(privacy.require_covers().await.is_err());
        privacy.lock().await.unwrap();
        assert!(grant.ensure().is_err());
        assert!(privacy.access().await.is_err());
        privacy.unlock().await.unwrap();
        let next = privacy.access().await.unwrap();
        assert!(next.ensure().is_ok());
        assert!(grant.ensure().is_err());
        privacy
            .save(AdultSettingsInput {
                entry_visible: false,
                covers_visible: false,
                remember_recent: false,
            })
            .await
            .unwrap();
        assert!(next.ensure().is_err());
    }
    #[tokio::test]
    async fn restart_never_restores_a_grant_and_disabled_recent_clears_history() {
        let (_dir, repo, privacy) = fixture().await;
        privacy
            .save(AdultSettingsInput {
                entry_visible: true,
                covers_visible: true,
                remember_recent: true,
            })
            .await
            .unwrap();
        privacy.unlock().await.unwrap();
        let now = chrono::Utc::now().timestamp_millis();
        let media = Uuid::new_v4();
        sqlx::query("INSERT INTO media_items(id,kind,title,adult_isolated,created_at,updated_at) VALUES (?,'adult_video','Synthetic private metadata',1,?,?)").bind(media.to_string()).bind(now).bind(now).execute(repo.pool()).await.unwrap();
        privacy.record_visit(media).await.unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM adult_recent_visits")
            .fetch_one(repo.pool())
            .await
            .unwrap();
        assert_eq!(count, 1);
        privacy
            .save(AdultSettingsInput {
                entry_visible: true,
                covers_visible: true,
                remember_recent: false,
            })
            .await
            .unwrap();
        let count: i64 = sqlx::query_scalar("SELECT count(*) FROM adult_recent_visits")
            .fetch_one(repo.pool())
            .await
            .unwrap();
        assert_eq!(count, 0);
        let restarted = AdultPrivacyService::new(repo.clone(), privacy.providers.clone());
        assert!(!restarted.settings().await.unwrap().unlocked);
        assert!(restarted.access().await.is_err());
        assert!(restarted.record_visit(media).await.is_err());
    }
    #[tokio::test]
    async fn ordinary_snapshot_excludes_private_tasks_requests_and_provider_health() {
        use crate::adult_providers::AdultProviderHub;
        use mediaatlas_domain::{MediaKind, ProviderJob};
        let (_dir, repo, privacy) = fixture().await;
        for descriptor in AdultProviderHub::descriptors() {
            privacy
                .providers
                .register_adult_descriptor(descriptor)
                .await
                .unwrap();
        }
        let id = Uuid::new_v4();
        let now = chrono::Utc::now();
        let descriptor = AdultProviderHub::descriptors().remove(0);
        let job = ProviderJob {
            id,
            provider_id: descriptor.id.clone(),
            media_item_id: None,
            media_kind: Some(MediaKind::AdultVideo),
            file_identity: None,
            region: None,
            correlation_id: id,
            operation: "search".into(),
            query: "SYNTHETIC-PRIVATE-001".into(),
            language: "en-US".into(),
            config: Default::default(),
            created_at: now,
        };
        sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES (?,'search_providers','waiting_for_user','{\"adult\":true}',?,?)").bind(id.to_string()).bind(now.timestamp_millis()).bind(now.timestamp_millis()).execute(repo.pool()).await.unwrap();
        sqlx::query("INSERT INTO provider_sessions(task_id,provider_id,job_json) VALUES (?,'adult_scraper',?)").bind(id.to_string()).bind(serde_json::to_string(&job).unwrap()).execute(repo.pool()).await.unwrap();
        for descriptor in crate::video_providers::VideoProviderHub::descriptors() {
            privacy
                .providers
                .register_adapter(std::sync::Arc::new(
                    crate::providers::TypedVideoRegistration(descriptor),
                ))
                .await
                .unwrap();
        }
        for id in [
            "fixture.offline",
            "tmdb",
            "anilist",
            "wikipedia.html",
            "dmm",
            "wikidata",
        ] {
            let injected = mediaatlas_domain::ProviderConfig {
                credential_ref: Some("arbitrary.auth.ref".into()),
                ..Default::default()
            };
            assert_eq!(
                privacy.providers.update_config(id, &injected).await,
                Err(RepositoryError::InvalidInput)
            );
            let mut limits = privacy.providers.config(id).await.unwrap();
            limits.timeout_ms = 10000;
            privacy.providers.update_config(id, &limits).await.unwrap();
        }
        let ordinary = privacy.providers.snapshot().await.unwrap();
        let serialized = serde_json::to_string(&ordinary).unwrap();
        assert!(!serialized.contains("SYNTHETIC-PRIVATE"));
        assert!(!ordinary
            .providers
            .iter()
            .any(|p| p.descriptor.media_kinds.contains(&MediaKind::AdultVideo)));
        assert!(ordinary.tasks.is_empty());
        assert_eq!(
            privacy
                .providers
                .adult_snapshot()
                .await
                .unwrap()
                .tasks
                .len(),
            1
        );
        assert_eq!(
            privacy
                .providers
                .start(&descriptor.id, "Synthetic query")
                .await,
            Err(RepositoryError::InvalidInput)
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdultRecentVisit {
    pub media_id: Uuid,
    pub title: String,
    pub visited_at: chrono::DateTime<chrono::Utc>,
}
impl AdultPrivacyService {
    pub async fn recent(&self) -> Result<Vec<AdultRecentVisit>, RepositoryError> {
        let access = self.access().await?;
        if !self.settings().await?.remember_recent {
            return Ok(vec![]);
        }
        let rows=sqlx::query("SELECT v.media_item_id,m.title,v.visited_at FROM adult_recent_visits v JOIN media_items m ON m.id=v.media_item_id WHERE m.kind='adult_video' AND m.adult_isolated=1 AND EXISTS(SELECT 1 FROM media_files f JOIN library_roots r ON r.id=f.root_id WHERE f.media_item_id=m.id AND r.kind='adult_video' AND r.enabled=1) ORDER BY visited_at DESC LIMIT 32").fetch_all(self.repo.pool()).await?;
        let result = rows
            .into_iter()
            .map(|row| {
                Ok(AdultRecentVisit {
                    media_id: Uuid::parse_str(&row.try_get::<String, _>("media_item_id")?)
                        .map_err(|_| RepositoryError::CorruptData)?,
                    title: row.try_get("title")?,
                    visited_at: chrono::DateTime::from_timestamp_millis(row.try_get("visited_at")?)
                        .ok_or(RepositoryError::CorruptData)?,
                })
            })
            .collect::<Result<Vec<_>, RepositoryError>>()?;
        access.ensure()?;
        Ok(result)
    }
}

#[cfg(test)]
mod race_tests {
    use super::*;
    use crate::{browse::BrowseService, network::NetworkEngine};
    use mediaatlas_application::library::RootInput;
    use mediaatlas_domain::MediaKind;
    #[tokio::test]
    async fn disabling_recent_concurrently_with_a_visit_never_leaves_disabled_history() {
        let dir = tempfile::tempdir().unwrap();
        let repo = LibraryRepository::open(&dir.path().join("synthetic.sqlite"))
            .await
            .unwrap();
        let providers = ProviderService::with_engine(repo.clone(), NetworkEngine::for_loopback(1))
            .await
            .unwrap();
        let privacy = AdultPrivacyService::new(repo.clone(), providers);
        let media = Uuid::new_v4();
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query("INSERT INTO media_items(id,kind,title,adult_isolated,created_at,updated_at) VALUES (?,'adult_video','Synthetic visit',1,?,?)").bind(media.to_string()).bind(now).bind(now).execute(repo.pool()).await.unwrap();
        for _ in 0..20 {
            privacy
                .save(AdultSettingsInput {
                    entry_visible: true,
                    covers_visible: false,
                    remember_recent: true,
                })
                .await
                .unwrap();
            privacy.unlock().await.unwrap();
            let visit = privacy.record_visit(media);
            let disable = privacy.save(AdultSettingsInput {
                entry_visible: true,
                covers_visible: false,
                remember_recent: false,
            });
            let (visit, disable) = tokio::join!(visit, disable);
            visit.unwrap();
            disable.unwrap();
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM adult_recent_visits")
                .fetch_one(repo.pool())
                .await
                .unwrap();
            assert_eq!(count, 0);
            assert!(privacy.recent().await.unwrap().is_empty());
        }
    }
    #[tokio::test]
    async fn ordinary_and_adult_roots_cannot_overlap_in_either_direction() {
        let dir = tempfile::tempdir().unwrap();
        let repo = LibraryRepository::open(&dir.path().join("synthetic.sqlite"))
            .await
            .unwrap();
        let browse = BrowseService::new(repo, dir.path().join("cache"));
        let parent = dir.path().join("synthetic-media");
        let child = parent.join("private");
        std::fs::create_dir_all(&child).unwrap();
        let input = |path: &std::path::Path, kind| RootInput {
            id: None,
            name: "Synthetic root".into(),
            path: path.to_string_lossy().into_owned(),
            kind: Some(kind),
            enabled: true,
            exclude_patterns: vec![],
        };
        let general = browse
            .save_root(&input(&parent, MediaKind::Movie))
            .await
            .unwrap();
        assert!(matches!(
            browse
                .save_root(&input(&child, MediaKind::AdultVideo))
                .await,
            Err(RepositoryError::ProtectedMetadata)
        ));
        browse.remove_root(general.root.id).await.unwrap();
        browse
            .save_root(&input(&child, MediaKind::AdultVideo))
            .await
            .unwrap();
        assert!(matches!(
            browse.save_root(&input(&parent, MediaKind::Movie)).await,
            Err(RepositoryError::ProtectedMetadata)
        ));
    }
}

#[cfg(test)]
mod output_revocation_tests {
    use super::*;
    use crate::{browse::BrowseService, media_output::MediaOutputService, network::NetworkEngine};
    use mediaatlas_application::library::RootInput;
    use mediaatlas_domain::{MediaFile, MediaItem, MediaKind};
    #[tokio::test]
    async fn lock_invalidates_old_output_even_after_a_fresh_unlock() {
        let dir = tempfile::tempdir().unwrap();
        let repo = LibraryRepository::open(&dir.path().join("synthetic.sqlite"))
            .await
            .unwrap();
        let providers = ProviderService::with_engine(repo.clone(), NetworkEngine::for_loopback(1))
            .await
            .unwrap();
        let privacy = AdultPrivacyService::new(repo.clone(), providers);
        privacy
            .save(AdultSettingsInput {
                entry_visible: true,
                covers_visible: false,
                remember_recent: false,
            })
            .await
            .unwrap();
        privacy.unlock().await.unwrap();
        let path = dir.path().join("synthetic-media");
        std::fs::create_dir(&path).unwrap();
        let source = path.join("SYNTHETIC-001.mkv");
        std::fs::write(&source, b"synthetic video, not real media").unwrap();
        let browse = BrowseService::new(repo.clone(), dir.path().join("thumbs"));
        let root = browse
            .save_root(&RootInput {
                id: None,
                name: "Synthetic adult fixture".into(),
                path: path.to_string_lossy().into_owned(),
                kind: Some(MediaKind::AdultVideo),
                enabled: true,
                exclude_patterns: vec![],
            })
            .await
            .unwrap();
        let now = chrono::Utc::now();
        let media = MediaItem {
            id: Uuid::new_v4(),
            kind: MediaKind::AdultVideo,
            title: "Synthetic metadata".into(),
            original_title: None,
            year: None,
            created_at: now,
            updated_at: now,
        };
        repo.save_media(&media).await.unwrap();
        repo.save_file(&MediaFile {
            id: Uuid::new_v4(),
            media_item_id: media.id,
            root_id: root.root.id,
            relative_path: "SYNTHETIC-001.mkv".into(),
            size_bytes: std::fs::metadata(&source).unwrap().len(),
            modified_at: None,
            part: None,
        })
        .await
        .unwrap();
        let cache = dir.path().join("adult-cache");
        let output = MediaOutputService::new_adult(
            repo.clone(),
            cache.clone(),
            privacy.access().await.unwrap(),
        );
        // Queue a preview behind a real SQLite writer, then revoke it while
        // both the preview and lock's durable invalidation are waiting to write.
        let mut writer = repo.pool().begin().await.unwrap();
        sqlx::query("UPDATE adult_privacy_settings SET covers_visible=covers_visible WHERE id=1")
            .execute(&mut *writer)
            .await
            .unwrap();
        let pending = output.preview(media.id);
        tokio::pin!(pending);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(25), &mut pending)
                .await
                .is_err()
        );
        let locking = privacy.lock();
        tokio::pin!(locking);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(25), &mut locking)
                .await
                .is_err()
        );
        assert!(privacy.access().await.is_err());
        writer.commit().await.unwrap();
        let (pending, locking) = tokio::join!(pending, locking);
        assert!(pending.is_err());
        locking.unwrap();
        let uncommitted: i64 = sqlx::query_scalar("SELECT count(*) FROM output_previews")
            .fetch_one(repo.pool())
            .await
            .unwrap();
        assert_eq!(uncommitted, 0);
        privacy.unlock().await.unwrap();
        let output = MediaOutputService::new_adult(
            repo.clone(),
            cache.clone(),
            privacy.access().await.unwrap(),
        );
        let plan = output.preview(media.id).await.unwrap();
        privacy.lock().await.unwrap();
        privacy.unlock().await.unwrap();
        let fresh =
            MediaOutputService::new_adult(repo.clone(), cache, privacy.access().await.unwrap());
        assert_eq!(
            fresh.execute(plan.preview_id).await.unwrap_err(),
            RepositoryError::Conflict
        );
        assert!(!path.join("SYNTHETIC-001.nfo").exists());
        let newplan = fresh.preview(media.id).await.unwrap();
        assert!(fresh.execute(newplan.preview_id).await.is_ok());
        assert_eq!(
            std::fs::read(source).unwrap(),
            b"synthetic video, not real media"
        );
    }
}

#[cfg(test)]
mod terminal_request_tests {
    use super::*;
    use crate::network::{CancellationToken, NetworkEngine};
    #[tokio::test]
    async fn lock_cancels_gallery_requests_from_an_already_succeeded_adult_run() {
        let dir = tempfile::tempdir().unwrap();
        let repo = LibraryRepository::open(&dir.path().join("synthetic.sqlite"))
            .await
            .unwrap();
        let providers = ProviderService::with_engine(repo.clone(), NetworkEngine::for_loopback(1))
            .await
            .unwrap();
        let privacy = AdultPrivacyService::new(repo.clone(), providers.clone());
        privacy
            .save(AdultSettingsInput {
                entry_visible: true,
                covers_visible: true,
                remember_recent: false,
            })
            .await
            .unwrap();
        privacy.unlock().await.unwrap();
        let id = Uuid::new_v4();
        let now = chrono::Utc::now().timestamp_millis();
        sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES (?,'search_providers','succeeded','{\"adult\":true}',?,?)").bind(id.to_string()).bind(now).bind(now).execute(repo.pool()).await.unwrap();
        sqlx::query("INSERT INTO adult_scrape_sessions(task_id,query_json,jobs_json,created_at) VALUES (?,'{}','{}',?)").bind(id.to_string()).bind(now).execute(repo.pool()).await.unwrap();
        let token = providers
            .attach_cancellation(id, CancellationToken::new())
            .await;
        privacy.lock().await.unwrap();
        assert!(token.is_cancelled());
        let state: String = sqlx::query_scalar("SELECT state FROM scrape_tasks WHERE id=?")
            .bind(id.to_string())
            .fetch_one(repo.pool())
            .await
            .unwrap();
        assert_eq!(state, "succeeded");
        providers.detach_cancellation(id).await;
    }
}
