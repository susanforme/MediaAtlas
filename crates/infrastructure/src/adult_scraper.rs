//! Independent adult catalog review and automatic refresh, with revocable access.
//! Desktop composition supplies an adult-only cache and checks its unlock grant before each call.
pub(crate) mod automatic;

use crate::{
    adult_providers::AdultProviderHub,
    library::{LibraryRepository, RepositoryError},
    network::{CancellationToken, NetworkEngine},
    providers::ProviderService,
    scraper::ArtworkPreview,
};
use chrono::Utc;
use mediaatlas_domain::*;
use serde::{Deserialize, Serialize};
use sqlx::{Row, SqliteConnection};
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::Mutex;
use uuid::Uuid;

type Result<T> = std::result::Result<T, RepositoryError>;

#[derive(Clone)]
pub struct AdultScraperService {
    repo: LibraryRepository,
    providers: ProviderService,
    hub: Arc<AdultProviderHub>,
    engine: Arc<NetworkEngine>,
    cache: PathBuf,
    gate: Arc<Mutex<()>>,
    auth: Option<crate::provider_auth::ProviderAuthService>,
    access: Option<crate::media_output::AdultOutputAccess>,
    automatic_artwork: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdultHistoryEntry {
    pub id: Uuid,
    pub query: AdultQuery,
    pub state: String,
    pub created_at: chrono::DateTime<Utc>,
    pub candidate_count: usize,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct RootSnapshot {
    file_id: String,
    root_id: String,
    path: String,
    relative_path: String,
}
impl AdultScraperService {
    pub fn new(
        repo: LibraryRepository,
        providers: ProviderService,
        hub: Arc<AdultProviderHub>,
        engine: Arc<NetworkEngine>,
        cache: PathBuf,
    ) -> Self {
        Self {
            repo,
            providers,
            hub,
            engine,
            cache,
            gate: Arc::default(),
            auth: None,
            access: None,
            automatic_artwork: false,
        }
    }
    pub fn with_auth_service(mut self, auth: crate::provider_auth::ProviderAuthService) -> Self {
        self.auth = Some(auth);
        self
    }
    pub fn with_access(mut self, access: crate::media_output::AdultOutputAccess) -> Self {
        self.access = Some(access);
        self
    }
    pub fn with_automatic_artwork(mut self, enabled: bool) -> Self {
        self.automatic_artwork = enabled;
        self
    }
    fn ensure_access(&self) -> Result<()> {
        if let Some(access) = &self.access {
            access.ensure()?;
        }
        Ok(())
    }
    fn artwork_service(&self) -> crate::media_output::MediaOutputService {
        crate::media_output::MediaOutputService::new_adult(
            self.repo.clone(),
            self.cache.clone(),
            self.access
                .clone()
                .unwrap_or_else(crate::media_output::AdultOutputAccess::new),
        )
    }
    pub async fn initialize(&self) -> Result<()> {
        sqlx::query("UPDATE adult_merge_previews SET state='interrupted' WHERE state='running'")
            .execute(self.repo.pool())
            .await?;
        Ok(())
    }
    pub async fn search(&self, request: AdultSearchRequest) -> Result<AdultSearchPreview> {
        self.search_cancellable(request, CancellationToken::new())
            .await
    }
    async fn search_cancellable(
        &self,
        request: AdultSearchRequest,
        cancellation: CancellationToken,
    ) -> Result<AdultSearchPreview> {
        self.ensure_access()?;
        validate_query(&request.query)?;
        if request.providers.is_empty() || request.providers.len() > 2 {
            return Err(RepositoryError::InvalidInput);
        }
        let mut seen = HashSet::new();
        let mut jobs = BTreeMap::new();
        let id = Uuid::new_v4();
        let now = Utc::now();
        for provider in &request.providers {
            if !seen.insert(provider)
                || !AdultProviderHub::descriptors()
                    .iter()
                    .any(|d| &d.id == provider)
            {
                return Err(RepositoryError::InvalidInput);
            }
            let mut config = self.providers.config(provider).await?;
            if let Some(auth) = &self.auth {
                config.credential_ref = auth.snapshot_ref(provider).await?;
            }
            jobs.insert(
                provider.clone(),
                ProviderJob {
                    id,
                    provider_id: provider.clone(),
                    media_item_id: None,
                    media_kind: Some(MediaKind::AdultVideo),
                    file_identity: None,
                    region: None,
                    correlation_id: id,
                    operation: "search".into(),
                    query: request.query.catalog_number.clone(),
                    language: request.query.language.clone(),
                    config,
                    created_at: now,
                },
            );
        }
        let first = jobs.values().next().ok_or(RepositoryError::InvalidInput)?;
        let mut tx = self.repo.pool().begin().await?;
        sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES (?,'search_providers','running',?,?,?)").bind(id.to_string()).bind(json(&serde_json::json!({"adult":true,"request":request}))?).bind(now.timestamp_millis()).bind(now.timestamp_millis()).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO provider_sessions(task_id,provider_id,job_json) VALUES (?,'adult_scraper',?)").bind(id.to_string()).bind(json(first)?).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO adult_scrape_sessions(task_id,query_json,jobs_json,created_at) VALUES (?,?,?,?)").bind(id.to_string()).bind(json(&request.query)?).bind(json(&jobs)?).bind(now.timestamp_millis()).execute(&mut *tx).await?;
        for job in jobs.values() {
            sqlx::query("INSERT INTO scrape_runs(id,task_id,provider_id,config_snapshot,state,started_at) VALUES (?,?,?,?,'running',?)").bind(Uuid::new_v4().to_string()).bind(id.to_string()).bind(&job.provider_id).bind(json(&job.config)?).bind(now.timestamp_millis()).execute(&mut *tx).await?;
        }
        self.ensure_access()?;
        tx.commit().await?;
        let cancel = self.providers.attach_cancellation(id, cancellation).await;
        let fetched = self.fetch_search(&request, &jobs, id, &cancel).await;
        self.providers.detach_cancellation(id).await;
        if fetched.is_err() && cancel.is_cancelled() {
            let _ = self.providers.cancel(id).await;
        }
        fetched
    }
    async fn fetch_search(
        &self,
        request: &AdultSearchRequest,
        jobs: &BTreeMap<String, ProviderJob>,
        id: Uuid,
        cancel: &CancellationToken,
    ) -> Result<AdultSearchPreview> {
        self.ensure_access()?;
        let mut result = AdultSearchPreview {
            schema_version: 1,
            run_id: id,
            candidates: vec![],
            failures: vec![],
        };
        let mut pending = tokio::task::JoinSet::new();
        for job in jobs.values() {
            let hub = self.hub.clone();
            let query = request.query.clone();
            let job = job.clone();
            let cancel = cancel.clone();
            pending.spawn(async move {
                let outcome = tokio::time::timeout(
                    std::time::Duration::from_millis(job.config.timeout_ms.max(1)),
                    hub.search(&job.provider_id, &query, &job, &cancel),
                )
                .await
                .unwrap_or(Err(ProviderFailure::Timeout));
                (job.provider_id, outcome)
            });
        }
        while let Some(outcome) = pending.join_next().await {
            let (provider_id, fetched) = outcome.map_err(|_| RepositoryError::InvalidInput)?;
            match fetched {
                Ok(candidates) if candidates.len() <= 50 => {
                    for candidate in candidates {
                        if candidate.provider_id != provider_id
                            || validate_candidate(&candidate).is_err()
                        {
                            result.failures.push(ProviderSearchFailure {
                                provider_id: provider_id.clone(),
                                failure: ProviderFailure::IdentityMismatch,
                            });
                            continue;
                        }
                        result.candidates.push(match_adult_candidate(
                            &request.query,
                            candidate,
                            &result.candidates,
                        ));
                    }
                }
                Ok(_) => result.failures.push(ProviderSearchFailure {
                    provider_id,
                    failure: ProviderFailure::ResponseTooLarge,
                }),
                Err(failure) => result.failures.push(ProviderSearchFailure {
                    provider_id,
                    failure,
                }),
            }
        }
        self.persist_records(id).await?;
        self.ensure_access()?;
        if cancel.is_cancelled() {
            return Err(RepositoryError::Provider(ProviderFailure::Cancelled));
        }
        let mut tx = self.repo.pool().begin().await?;
        let updated = sqlx::query("UPDATE scrape_tasks SET state='waiting_for_user',updated_at=MAX(updated_at,?) WHERE id=? AND state='running'").bind(Utc::now().timestamp_millis()).bind(id.to_string()).execute(&mut *tx).await?;
        if updated.rows_affected() != 1 {
            return Err(RepositoryError::Conflict);
        }
        sqlx::query("UPDATE adult_scrape_sessions SET search_json=? WHERE task_id=?")
            .bind(json(&result)?)
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?;
        for job in jobs.values() {
            let failed = result
                .failures
                .iter()
                .any(|f| f.provider_id == job.provider_id);
            sqlx::query("UPDATE scrape_runs SET state=?,finished_at=MAX(started_at,?) WHERE task_id=? AND provider_id=?").bind(if failed {"failed"} else {"succeeded"}).bind(Utc::now().timestamp_millis()).bind(id.to_string()).bind(&job.provider_id).execute(&mut *tx).await?;
        }
        self.ensure_access()?;
        tx.commit().await?;
        self.ensure_access()?;
        Ok(result)
    }
    async fn session(
        &self,
        id: Uuid,
    ) -> Result<(
        AdultQuery,
        BTreeMap<String, ProviderJob>,
        AdultSearchPreview,
    )> {
        self.ensure_access()?;
        let row = sqlx::query("SELECT v.query_json,v.jobs_json,v.search_json,t.state FROM adult_scrape_sessions v JOIN scrape_tasks t ON t.id=v.task_id WHERE task_id=?").bind(id.to_string()).fetch_optional(self.repo.pool()).await?.ok_or(RepositoryError::NotFound)?;
        if !matches!(
            row.try_get::<String, _>("state")?.as_str(),
            "waiting_for_user" | "succeeded"
        ) {
            return Err(RepositoryError::Conflict);
        }
        self.ensure_access()?;
        Ok((
            decode(&row.try_get::<String, _>("query_json")?)?,
            decode(&row.try_get::<String, _>("jobs_json")?)?,
            decode(
                &row.try_get::<Option<String>, _>("search_json")?
                    .ok_or(RepositoryError::Conflict)?,
            )?,
        ))
    }
    pub async fn search_preview(&self, id: Uuid) -> Result<AdultSearchPreview> {
        Ok(self.session(id).await?.2)
    }
    pub async fn detail(&self, run_id: Uuid, index: usize) -> Result<AdultDetail> {
        self.detail_cancellable(run_id, index, CancellationToken::new())
            .await
    }
    async fn detail_cancellable(
        &self,
        run_id: Uuid,
        index: usize,
        cancellation: CancellationToken,
    ) -> Result<AdultDetail> {
        let (query, jobs, search) = self.session(run_id).await?;
        let candidate = &search
            .candidates
            .get(index)
            .ok_or(RepositoryError::InvalidInput)?
            .candidate;
        validate_query_identity(&query, candidate)?;
        if let Some(data) = sqlx::query_scalar::<_, String>(
            "SELECT detail_json FROM adult_scrape_details WHERE task_id=? AND candidate_index=?",
        )
        .bind(run_id.to_string())
        .bind(index as i64)
        .fetch_optional(self.repo.pool())
        .await?
        {
            let detail: AdultDetail = decode(&data)?;
            validate_detail(candidate, &detail)?;
            self.ensure_access()?;
            return Ok(detail);
        }
        let mut job = jobs
            .get(&candidate.provider_id)
            .ok_or(RepositoryError::CorruptData)?
            .clone();
        job.operation = "detail".into();
        let cancel = self
            .providers
            .attach_cancellation(run_id, cancellation)
            .await;
        let result = self
            .fetch_detail(run_id, index, candidate, &job, &cancel)
            .await;
        self.providers.detach_cancellation(run_id).await;
        result
    }
    async fn fetch_detail(
        &self,
        run_id: Uuid,
        index: usize,
        candidate: &AdultCandidate,
        job: &ProviderJob,
        cancel: &CancellationToken,
    ) -> Result<AdultDetail> {
        self.session(run_id).await?;
        let detail = tokio::time::timeout(
            std::time::Duration::from_millis(job.config.timeout_ms.max(1)),
            self.hub.detail(candidate, job, cancel),
        )
        .await
        .unwrap_or(Err(ProviderFailure::Timeout));
        self.persist_records(run_id).await?;
        let detail = detail.map_err(RepositoryError::Provider)?;
        validate_detail(candidate, &detail)?;
        let mut tx = self.repo.pool().begin().await?;
        let state: String = sqlx::query_scalar("SELECT state FROM scrape_tasks WHERE id=?")
            .bind(run_id.to_string())
            .fetch_one(&mut *tx)
            .await?;
        if !matches!(state.as_str(), "waiting_for_user" | "succeeded") || cancel.is_cancelled() {
            return Err(RepositoryError::Conflict);
        }
        sqlx::query("INSERT OR IGNORE INTO adult_scrape_details(task_id,candidate_index,detail_json) VALUES (?,?,?)").bind(run_id.to_string()).bind(index as i64).bind(json(&detail)?).execute(&mut *tx).await?;
        let persisted: String = sqlx::query_scalar(
            "SELECT detail_json FROM adult_scrape_details WHERE task_id=? AND candidate_index=?",
        )
        .bind(run_id.to_string())
        .bind(index as i64)
        .fetch_one(&mut *tx)
        .await?;
        self.ensure_access()?;
        tx.commit().await?;
        decode(&persisted)
    }
    pub async fn artwork_preview(
        &self,
        run_id: Uuid,
        index: usize,
        artwork_index: usize,
    ) -> Result<ArtworkPreview> {
        use base64::Engine;
        let detail = self.detail(run_id, index).await?;
        let (_, jobs, _) = self.session(run_id).await?;
        let mut job = jobs
            .get(&detail.identity.provider_id)
            .ok_or(RepositoryError::CorruptData)?
            .clone();
        job.operation = "adult_artwork_preview".into();
        let art = detail
            .artwork
            .get(artwork_index)
            .ok_or(RepositoryError::InvalidInput)?;
        let service = self.artwork_service();
        let cancel = self
            .providers
            .attach_cancellation(run_id, CancellationToken::new())
            .await;
        let result = async {
            self.session(run_id).await?;
            let cached = service
                .cache_artwork(&self.engine, &job, &art.url, &cancel)
                .await;
            self.persist_records(run_id).await?;
            self.session(run_id).await?;
            self.ensure_access()?;
            if cancel.is_cancelled() {
                return Err(RepositoryError::Provider(ProviderFailure::Cancelled));
            }
            let bytes = service.thumbnail_bytes(&cached.map_err(RepositoryError::Provider)?)?;
            Ok(ArtworkPreview {
                thumbnail: format!(
                    "data:image/png;base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(bytes)
                ),
            })
        }
        .await;
        self.providers.detach_cancellation(run_id).await;
        result
    }
    pub async fn preview(&self, request: AdultMergeRequest) -> Result<AdultMergePreview> {
        let detail = self.detail(request.run_id, request.candidate_index).await?;
        let media = self.adult_media(request.media_item_id).await?;
        let mut connection = self.repo.pool().acquire().await?;
        let roots = root_snapshots(&mut connection, media.id).await?;
        drop(connection);
        let old = self.repo.list_metadata_values(media.id).await?;
        let remote = adult_fields(&detail)?;
        let keys: HashSet<_> = request.selected_fields.iter().collect();
        if keys.len() != request.selected_fields.len()
            || keys.iter().any(|k| !remote.iter().any(|f| &f.key == *k))
        {
            return Err(RepositoryError::InvalidInput);
        }
        let mut fields = vec![];
        for field in remote {
            let language = normalize_locale(field.language.as_deref())
                .map_err(|_| RepositoryError::InvalidInput)?;
            let previous = old
                .iter()
                .find(|v| {
                    v.field_key == field.key
                        && normalize_locale(v.locale.as_deref()).ok() == Some(language.clone())
                })
                .cloned();
            let protected = previous.as_ref().is_some_and(MetadataValue::is_protected);
            fields.push(FieldDiff {
                key: field.key.clone(),
                language,
                old: previous.clone(),
                new: field.clone(),
                provider_id: detail.identity.provider_id.clone(),
                source_url: detail.identity.source_url.clone(),
                protected,
                needs_review: previous.as_ref().is_some_and(|v| v.value != field.value),
                selected: keys.contains(&field.key) && !protected && !empty_value(&field.value),
            });
        }
        let mut artwork = vec![];
        let mut indices = HashSet::new();
        for index in &request.selected_artwork {
            if !indices.insert(index) {
                return Err(RepositoryError::InvalidInput);
            }
            artwork.push(
                detail
                    .artwork
                    .get(*index)
                    .ok_or(RepositoryError::InvalidInput)?
                    .clone(),
            );
        }
        let preview = AdultMergePreview {
            schema_version: 1,
            preview_id: Uuid::new_v4(),
            media_item_id: media.id,
            fields,
            artwork,
            confirmed: false,
        };
        self.ensure_access()?;
        sqlx::query("INSERT INTO adult_merge_previews(id,task_id,media_item_id,preview_json,detail_json,expected_media_json,expected_roots_json,state,created_at) VALUES (?,?,?,?,?,?,?,'planned',?)").bind(preview.preview_id.to_string()).bind(request.run_id.to_string()).bind(media.id.to_string()).bind(json(&preview)?).bind(json(&detail)?).bind(json(&media)?).bind(json(&roots)?).bind(Utc::now().timestamp_millis()).execute(self.repo.pool()).await?;
        Ok(preview)
    }
    pub async fn confirm(&self, id: Uuid) -> Result<MergeResult> {
        self.confirm_inner(id, None).await
    }
    async fn confirm_inner(
        &self,
        id: Uuid,
        automatic: Option<&crate::scraper::automatic::AutomaticContext>,
    ) -> Result<MergeResult> {
        self.ensure_access()?;
        let _gate = self.gate.lock().await;
        let row = sqlx::query("SELECT * FROM adult_merge_previews WHERE id=?")
            .bind(id.to_string())
            .fetch_optional(self.repo.pool())
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let preview: AdultMergePreview = decode(&row.try_get::<String, _>("preview_json")?)?;
        let detail: AdultDetail = decode(&row.try_get::<String, _>("detail_json")?)?;
        let media = self.adult_media(preview.media_item_id).await?;
        if row.try_get::<String, _>("state")? == "succeeded" {
            let mut result: MergeResult = decode(&row.try_get::<String, _>("result_json")?)?;
            result.already_applied = true;
            return Ok(result);
        }
        let expected: MediaItem = decode(&row.try_get::<String, _>("expected_media_json")?)?;
        let roots: Vec<RootSnapshot> = decode(&row.try_get::<String, _>("expected_roots_json")?)?;
        if expected != media {
            return Err(RepositoryError::Conflict);
        }
        let run_id = Uuid::parse_str(&row.try_get::<String, _>("task_id")?)
            .map_err(|_| RepositoryError::CorruptData)?;
        let (_, jobs, _) = self.session(run_id).await?;
        let mut job = jobs
            .get(&detail.identity.provider_id)
            .ok_or(RepositoryError::CorruptData)?
            .clone();
        job.operation = "adult_artwork".into();
        let cancel = self
            .providers
            .attach_cancellation(
                run_id,
                automatic
                    .map(|context| context.cancel.clone())
                    .unwrap_or_default(),
            )
            .await;
        let result = self
            .confirm_with_token(
                id, run_id, &preview, &detail, &expected, &roots, &job, &cancel, automatic,
            )
            .await;
        self.providers.detach_cancellation(run_id).await;
        result
    }
    #[allow(clippy::too_many_arguments)]
    async fn confirm_with_token(
        &self,
        id: Uuid,
        run_id: Uuid,
        preview: &AdultMergePreview,
        detail: &AdultDetail,
        expected: &MediaItem,
        roots: &[RootSnapshot],
        job: &ProviderJob,
        cancel: &CancellationToken,
        automatic: Option<&crate::scraper::automatic::AutomaticContext>,
    ) -> Result<MergeResult> {
        self.session(run_id).await?;
        let mut artworks = vec![];
        let service = self.artwork_service();
        for art in &preview.artwork {
            let cached = service
                .cache_artwork(&self.engine, job, &art.url, cancel)
                .await;
            self.persist_records(run_id).await?;
            artworks.push((art.clone(), cached.map_err(RepositoryError::Provider)?));
        }
        let mut tx = self.repo.pool().begin().await?;
        if let Some(context) = automatic {
            context.validate_tx(&mut tx, expected.id).await?;
        }
        // This transaction acquires SQLite's write lock before reading mutable prerequisites.
        let changed=sqlx::query("UPDATE adult_merge_previews SET state='running' WHERE id=? AND state IN ('planned','interrupted')").bind(id.to_string()).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 {
            return Err(RepositoryError::Conflict);
        }
        let current = sqlx::query("SELECT * FROM media_items WHERE id=?")
            .bind(expected.id.to_string())
            .fetch_one(&mut *tx)
            .await?;
        if current.try_get::<String, _>("kind")? != "adult_video"
            || !current.try_get::<bool, _>("adult_isolated")?
            || current.try_get::<String, _>("title")? != expected.title
            || current.try_get::<Option<String>, _>("original_title")? != expected.original_title
            || current.try_get::<Option<i32>, _>("year")? != expected.year
            || current.try_get::<i64, _>("updated_at")? != expected.updated_at.timestamp_millis()
            || root_snapshots(&mut tx, expected.id).await? != roots
        {
            return Err(RepositoryError::Conflict);
        }
        let state: String = sqlx::query_scalar("SELECT state FROM scrape_tasks WHERE id=?")
            .bind(run_id.to_string())
            .fetch_one(&mut *tx)
            .await?;
        if state != "waiting_for_user" || cancel.is_cancelled() {
            return Err(RepositoryError::Conflict);
        }
        let mut applied = 0;
        let mut title = None;
        for diff in preview.fields.iter().filter(|f| f.selected) {
            let previous=sqlx::query("SELECT id,value_json,is_locked,is_user_modified FROM metadata_values WHERE media_item_id=? AND field_key=? AND locale=?").bind(expected.id.to_string()).bind(&diff.key).bind(diff.language.as_deref().unwrap_or("")).fetch_optional(&mut *tx).await?;
            if let Some(prev) = &previous {
                if prev.try_get::<bool, _>("is_locked")?
                    || prev.try_get::<bool, _>("is_user_modified")?
                {
                    return Err(RepositoryError::ProtectedMetadata);
                }
                if diff.old.as_ref().map(|v| json(&v.value)).transpose()?
                    != Some(prev.try_get("value_json")?)
                {
                    return Err(RepositoryError::Conflict);
                }
            } else if diff.old.is_some() {
                return Err(RepositoryError::Conflict);
            }
            if empty_value(&diff.new.value) {
                continue;
            }
            let value_id = previous
                .as_ref()
                .map(|v| v.try_get::<String, _>("id"))
                .transpose()?
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            sqlx::query("INSERT INTO metadata_values(id,media_item_id,field_key,locale,value_json,provider_id,source_url,fetched_at,is_user_modified,is_locked) VALUES (?,?,?,?,?,?,?,?,0,0) ON CONFLICT(id) DO UPDATE SET value_json=excluded.value_json,provider_id=excluded.provider_id,source_url=excluded.source_url,fetched_at=excluded.fetched_at WHERE metadata_values.is_locked=0 AND metadata_values.is_user_modified=0").bind(value_id).bind(expected.id.to_string()).bind(&diff.key).bind(diff.language.as_deref().unwrap_or("")).bind(json(&diff.new.value)?).bind(&diff.provider_id).bind(&diff.source_url).bind(Utc::now().timestamp_millis()).execute(&mut *tx).await?;
            if diff.key == "title" {
                title = diff.new.value.as_str().map(str::to_owned);
            }
            applied += 1;
        }
        let protected_title:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM metadata_values WHERE media_item_id=? AND field_key='title' AND (is_locked=1 OR is_user_modified=1))").bind(expected.id.to_string()).fetch_one(&mut *tx).await?;
        if protected_title {
            title = None;
        }
        sqlx::query("UPDATE media_items SET title=?,updated_at=MAX(updated_at,?) WHERE id=?")
            .bind(title.as_ref().unwrap_or(&expected.title))
            .bind(Utc::now().timestamp_millis())
            .bind(expected.id.to_string())
            .execute(&mut *tx)
            .await?;
        // Catalog identity is retained as an issuer-scoped identifier. Distinct issuers
        // and provider people remain distinct; no display-name Person upserts occur.
        let catalog = &detail.identity.catalog_number;
        let existing: Vec<String> = sqlx::query_scalar(
            "SELECT external_id FROM external_ids WHERE media_item_id=? AND namespace=?",
        )
        .bind(expected.id.to_string())
        .bind(&catalog.namespace)
        .fetch_all(&mut *tx)
        .await?;
        if existing.iter().any(|v| v != &catalog.normalized) {
            return Err(RepositoryError::Conflict);
        }
        sqlx::query("INSERT OR IGNORE INTO external_ids(id,media_item_id,provider_id,namespace,external_id,is_primary,created_at) VALUES (?,?,?,?,?,1,?)").bind(Uuid::new_v4().to_string()).bind(expected.id.to_string()).bind(&detail.identity.provider_id).bind(&catalog.namespace).bind(&catalog.normalized).bind(Utc::now().timestamp_millis()).execute(&mut *tx).await?;
        for (art, cached) in artworks {
            sqlx::query("INSERT INTO artworks(id,media_item_id,kind,cache_path,source_url,provider_id,width,height,content_hash,created_at) VALUES (?,?,?,?,?,?,?,?,?,?)").bind(Uuid::new_v4().to_string()).bind(expected.id.to_string()).bind(art.kind).bind(cached.cache_path).bind(art.url).bind(art.source_provider).bind(i64::from(cached.width)).bind(i64::from(cached.height)).bind(cached.content_hash).bind(Utc::now().timestamp_millis()).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO adult_metadata_sources(media_item_id,metadata_json,merge_preview_id,updated_at) VALUES (?,?,?,?) ON CONFLICT(media_item_id) DO UPDATE SET metadata_json=excluded.metadata_json,merge_preview_id=excluded.merge_preview_id,updated_at=excluded.updated_at").bind(expected.id.to_string()).bind(json(detail)?).bind(id.to_string()).bind(Utc::now().timestamp_millis()).execute(&mut *tx).await?;
        // Manual identification also resolves the corresponding automatic
        // exception, without exposing adult task details to ordinary views.
        sqlx::query("UPDATE library_refresh_items SET state='matched',reason='identified',run_id=?,updated_at=? WHERE media_item_id=? AND state IN ('pending','needs_review','failed')")
            .bind(run_id.to_string()).bind(Utc::now().timestamp_millis()).bind(expected.id.to_string()).execute(&mut *tx).await?;
        sqlx::query("UPDATE scan_sessions SET matched_count=(SELECT count(*) FROM library_refresh_items i WHERE i.task_id=scan_sessions.task_id AND i.state='matched'),review_count=(SELECT count(*) FROM library_refresh_items i WHERE i.task_id=scan_sessions.task_id AND i.state='needs_review'),metadata_failed_count=(SELECT count(*) FROM library_refresh_items i WHERE i.task_id=scan_sessions.task_id AND i.state='failed') WHERE mode='automatic' AND task_id IN (SELECT task_id FROM library_refresh_items WHERE media_item_id=?)")
            .bind(expected.id.to_string()).execute(&mut *tx).await?;
        sqlx::query("UPDATE scrape_tasks SET state='succeeded',updated_at=MAX(updated_at,?) WHERE state='waiting_for_user' AND id IN (SELECT s.task_id FROM scan_sessions s JOIN library_refresh_items i ON i.task_id=s.task_id WHERE i.media_item_id=? AND s.mode='automatic' AND s.review_count=0 AND s.metadata_failed_count=0 AND NOT EXISTS(SELECT 1 FROM scan_candidates c WHERE c.task_id=s.task_id AND c.status='pending'))")
            .bind(Utc::now().timestamp_millis()).bind(expected.id.to_string()).execute(&mut *tx).await?;
        let result = MergeResult {
            schema_version: 1,
            preview_id: id,
            applied_fields: applied,
            already_applied: false,
        };
        sqlx::query("UPDATE adult_merge_previews SET state='succeeded',result_json=? WHERE id=? AND state='running'").bind(json(&result)?).bind(id.to_string()).execute(&mut *tx).await?;
        let changed=sqlx::query("UPDATE scrape_tasks SET state='succeeded',updated_at=MAX(updated_at,?) WHERE id=? AND state='waiting_for_user'").bind(Utc::now().timestamp_millis()).bind(run_id.to_string()).execute(&mut *tx).await?;
        if changed.rows_affected() != 1 || cancel.is_cancelled() {
            return Err(RepositoryError::Conflict);
        }
        self.ensure_access()?;
        tx.commit().await?;
        self.ensure_access()?;
        Ok(result)
    }
    async fn adult_media(&self, id: Uuid) -> Result<MediaItem> {
        self.ensure_access()?;
        let media = self
            .repo
            .get_media(id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let isolated: bool =
            sqlx::query_scalar("SELECT adult_isolated FROM media_items WHERE id=?")
                .bind(id.to_string())
                .fetch_one(self.repo.pool())
                .await?;
        if media.kind != MediaKind::AdultVideo || !isolated {
            return Err(RepositoryError::InvalidInput);
        }
        let mut connection = self.repo.pool().acquire().await?;
        root_snapshots(&mut connection, id).await?;
        self.ensure_access()?;
        Ok(media)
    }
    async fn persist_records(&self, id: Uuid) -> Result<()> {
        for record in self.engine.take_records(id).await {
            sqlx::query("INSERT INTO provider_requests(task_id,record_json) VALUES (?,?)")
                .bind(id.to_string())
                .bind(json(&record)?)
                .execute(self.repo.pool())
                .await?;
        }
        Ok(())
    }
    pub async fn media_detail(&self, id: Uuid) -> Result<Option<AdultDetail>> {
        self.adult_media(id).await?;
        let data = sqlx::query_scalar::<_, String>(
            "SELECT metadata_json FROM adult_metadata_sources WHERE media_item_id=?",
        )
        .bind(id.to_string())
        .fetch_optional(self.repo.pool())
        .await?;
        self.ensure_access()?;
        data.map(|data| decode(&data)).transpose()
    }
    pub async fn history(&self) -> Result<Vec<AdultHistoryEntry>> {
        self.ensure_access()?;
        let rows=sqlx::query("SELECT v.*,t.state FROM adult_scrape_sessions v JOIN scrape_tasks t ON t.id=v.task_id ORDER BY v.created_at DESC LIMIT 100").fetch_all(self.repo.pool()).await?;
        self.ensure_access()?;
        rows.into_iter()
            .map(|r| {
                Ok(AdultHistoryEntry {
                    id: Uuid::parse_str(&r.try_get::<String, _>("task_id")?)
                        .map_err(|_| RepositoryError::CorruptData)?,
                    query: decode(&r.try_get::<String, _>("query_json")?)?,
                    state: r.try_get("state")?,
                    created_at: chrono::DateTime::from_timestamp_millis(r.try_get("created_at")?)
                        .ok_or(RepositoryError::CorruptData)?,
                    candidate_count: r
                        .try_get::<Option<String>, _>("search_json")?
                        .map(|s| decode::<AdultSearchPreview>(&s).map(|p| p.candidates.len()))
                        .transpose()?
                        .unwrap_or(0),
                })
            })
            .collect()
    }
}
async fn root_snapshots(connection: &mut SqliteConnection, id: Uuid) -> Result<Vec<RootSnapshot>> {
    let rows=sqlx::query("SELECT f.id AS file_id,f.root_id,r.path,f.relative_path,r.kind,r.enabled FROM media_files f JOIN library_roots r ON r.id=f.root_id WHERE f.media_item_id=? ORDER BY f.id").bind(id.to_string()).fetch_all(connection).await?;
    if rows.is_empty() {
        return Err(RepositoryError::InvalidInput);
    }
    rows.into_iter()
        .map(|r| {
            if r.try_get::<Option<String>, _>("kind")?.as_deref() != Some("adult_video")
                || !r.try_get::<bool, _>("enabled")?
            {
                return Err(RepositoryError::InvalidInput);
            }
            Ok(RootSnapshot {
                file_id: r.try_get("file_id")?,
                root_id: r.try_get("root_id")?,
                path: r.try_get("path")?,
                relative_path: r.try_get("relative_path")?,
            })
        })
        .collect()
}
fn json<T: Serialize>(value: &T) -> Result<String> {
    serde_json::to_string(value).map_err(|_| RepositoryError::InvalidInput)
}
fn decode<T: serde::de::DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_str(value).map_err(|_| RepositoryError::CorruptData)
}
fn empty_value(value: &serde_json::Value) -> bool {
    value.is_null()
        || value.as_str().is_some_and(|s| s.trim().is_empty())
        || value.as_array().is_some_and(Vec::is_empty)
}
fn validate_query(query: &AdultQuery) -> Result<()> {
    normalize_catalog_number(&query.catalog_number, &query.namespace)
        .map_err(RepositoryError::Provider)?;
    if normalize_locale(Some(&query.language)).is_err()
        || query.studio.as_ref().is_some_and(|v| {
            v.trim().is_empty() || v.len() > 256 || v.chars().any(char::is_control)
        })
    {
        return Err(RepositoryError::InvalidInput);
    }
    Ok(())
}
fn validate_candidate(candidate: &AdultCandidate) -> Result<()> {
    let number = normalize_catalog_number(
        &candidate.catalog_number.raw,
        &candidate.catalog_number.namespace,
    )
    .map_err(RepositoryError::Provider)?;
    if number.normalized != candidate.catalog_number.normalized
        || number.part != candidate.catalog_number.part
        || candidate.remote_id.is_empty()
        || candidate.remote_id.len() > 128
        || candidate.title.trim().is_empty()
        || candidate.title.len() > 4096
        || candidate.title.chars().any(char::is_control)
        || normalize_locale(Some(&candidate.language)).is_err()
    {
        return Err(RepositoryError::Provider(ProviderFailure::IdentityMismatch));
    }
    Ok(())
}
fn validate_query_identity(query: &AdultQuery, candidate: &AdultCandidate) -> Result<()> {
    let number = normalize_catalog_number(&query.catalog_number, &query.namespace)
        .map_err(RepositoryError::Provider)?;
    if number.normalized != candidate.catalog_number.normalized
        || number
            .part
            .as_ref()
            .zip(candidate.catalog_number.part.as_ref())
            .is_some_and(|(a, b)| a != b)
        || query.studio.as_ref().is_some_and(|studio| {
            candidate
                .studio
                .as_ref()
                .is_none_or(|v| !query_studio_matches(studio, v))
        })
    {
        return Err(RepositoryError::Provider(ProviderFailure::IdentityMismatch));
    }
    Ok(())
}
fn validate_detail(candidate: &AdultCandidate, detail: &AdultDetail) -> Result<()> {
    validate_candidate(&detail.identity)?;
    if candidate.provider_id != detail.identity.provider_id
        || candidate.remote_id != detail.identity.remote_id
        || candidate.catalog_number.namespace != detail.identity.catalog_number.namespace
        || candidate.catalog_number.normalized != detail.identity.catalog_number.normalized
        || candidate.catalog_number.part != detail.identity.catalog_number.part
        || candidate.studio.as_ref().is_some_and(|studio| {
            detail
                .identity
                .studio
                .as_ref()
                .is_none_or(|v| normalized(v) != normalized(studio))
        })
    {
        return Err(RepositoryError::Provider(ProviderFailure::IdentityMismatch));
    }
    if detail.performers.len() > 200
        || detail.directors.len() > 100
        || detail.artwork.len() > 100
        || json(detail)?.len() > 512_000
        || detail.runtime_minutes.is_some_and(|v| v > 1440)
        || detail
            .release_date
            .as_ref()
            .is_some_and(|v| chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d").is_err())
        || detail.performers.iter().chain(&detail.directors).any(|p| {
            p.provider_id != candidate.provider_id
                || p.remote_id.is_empty()
                || p.remote_id.len() > 128
                || p.name.is_empty()
                || p.name.len() > 512
                || p.aliases.len() > 64
        })
        || detail.artwork.iter().any(|a| {
            a.source_provider != candidate.provider_id
                || !["cover", "back_cover", "sample"].contains(&a.kind.as_str())
        })
    {
        return Err(RepositoryError::InvalidInput);
    }
    Ok(())
}
fn adult_fields(detail: &AdultDetail) -> Result<Vec<RemoteField>> {
    let language = Some(detail.identity.language.clone());
    let mut fields = vec![
        RemoteField {
            key: "title".into(),
            value: serde_json::json!(detail.identity.title),
            language,
        },
        RemoteField {
            key: "catalog_numbers".into(),
            value: serde_json::json!([detail.identity.catalog_number]),
            language: None,
        },
        RemoteField {
            key: "studio".into(),
            value: serde_json::json!(detail.identity.studio),
            language: None,
        },
        RemoteField {
            key: "label".into(),
            value: serde_json::json!(detail.label),
            language: None,
        },
        RemoteField {
            key: "series".into(),
            value: serde_json::json!(detail.series),
            language: None,
        },
        RemoteField {
            key: "release_date".into(),
            value: serde_json::json!(detail.release_date),
            language: None,
        },
        RemoteField {
            key: "runtime_minutes".into(),
            value: serde_json::json!(detail.runtime_minutes),
            language: None,
        },
        RemoteField {
            key: "performers".into(),
            value: serde_json::json!(detail.performers),
            language: None,
        },
        RemoteField {
            key: "directors".into(),
            value: serde_json::json!(detail.directors),
            language: None,
        },
    ];
    if detail.identity.provider_id == "wikidata" {
        fields.push(RemoteField {
            key: "source_license".into(),
            value: serde_json::json!("CC0 1.0; https://creativecommons.org/publicdomain/zero/1.0/"),
            language: None,
        });
        fields.push(RemoteField {
            key: "attribution".into(),
            value: serde_json::json!(format!(
                "Wikidata contributors; {}",
                detail.identity.source_url
            )),
            language: None,
        });
    }
    for field in &fields {
        if json(&field.value)?.len() > 256_000 {
            return Err(RepositoryError::InvalidInput);
        }
    }
    Ok(fields)
}
// A free-text search constraint can select a displayed publisher name. Once
// selected, detail and duplicate checks compare the complete provider identity.
fn query_studio_matches(query: &str, source: &str) -> bool {
    normalized(query) == normalized(source)
        || (!query.contains("(dmm:maker:")
            && source
                .rsplit_once(" (dmm:maker:")
                .is_some_and(|(name, id)| {
                    id.ends_with(')') && normalized(name) == normalized(query)
                }))
}
fn normalized(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}
/// Catalog and issuer identity only; title/year and Movie ratings are never consumed.
pub fn match_adult_candidate(
    query: &AdultQuery,
    candidate: AdultCandidate,
    previous: &[AdultMatchedCandidate],
) -> AdultMatchedCandidate {
    let number = normalize_catalog_number(&query.catalog_number, &query.namespace).ok();
    let exact = number
        .as_ref()
        .is_some_and(|n| n.normalized == candidate.catalog_number.normalized);
    let namespace = number
        .as_ref()
        .is_some_and(|n| n.namespace == candidate.catalog_number.namespace);
    let part_conflict = number
        .as_ref()
        .and_then(|n| n.part.as_ref())
        .zip(candidate.catalog_number.part.as_ref())
        .is_some_and(|(a, b)| a != b);
    let studio_conflict = query.studio.as_ref().is_some_and(|s| {
        candidate
            .studio
            .as_ref()
            .is_none_or(|v| !query_studio_matches(s, v))
    });
    let mut reasons = vec![if exact {
        "catalog_exact"
    } else {
        "catalog_conflict"
    }
    .into()];
    if !namespace {
        reasons.push("issuer_namespace_review".into());
    }
    if part_conflict {
        reasons.push("part_conflict".into());
    }
    if studio_conflict {
        reasons.push("studio_conflict".into());
    }
    reasons.push("manual_selection_required".into());
    let duplicate_of = previous.iter().position(|p| {
        p.candidate.catalog_number.namespace == candidate.catalog_number.namespace
            && p.candidate.catalog_number.normalized == candidate.catalog_number.normalized
            && p.candidate.catalog_number.part == candidate.catalog_number.part
            && p.candidate
                .studio
                .as_ref()
                .zip(candidate.studio.as_ref())
                .is_some_and(|(a, b)| normalized(a) == normalized(b))
    });
    AdultMatchedCandidate {
        candidate,
        reasons,
        needs_review: !exact || !namespace || part_conflict || studio_conflict,
        duplicate_of,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{credentials::CredentialStore, library::MetadataWriteMode};
    struct Setup {
        _dir: tempfile::TempDir,
        repo: LibraryRepository,
        service: AdultScraperService,
        providers: ProviderService,
        media: MediaItem,
        root: LibraryRoot,
        run: Uuid,
    }
    fn query() -> AdultQuery {
        AdultQuery {
            catalog_number: "SYN_001-CD1".into(),
            namespace: "synthetic.issuer".into(),
            studio: Some("Synthetic Studio".into()),
            language: "en-US".into(),
        }
    }
    fn candidate() -> AdultCandidate {
        AdultCandidate {
            provider_id: "dmm".into(),
            remote_id: "synthetic001".into(),
            catalog_number: normalize_catalog_number("SYN-001", "synthetic.issuer").unwrap(),
            title: "Synthetic catalog fixture".into(),
            studio: Some("Synthetic Studio".into()),
            source_url: "https://example.invalid/synthetic-001".into(),
            language: "en-US".into(),
        }
    }
    fn detail() -> AdultDetail {
        AdultDetail {
            identity: candidate(),
            label: Some("Synthetic Label".into()),
            series: Some("Synthetic Series".into()),
            release_date: Some("2020-01-02".into()),
            runtime_minutes: Some(90),
            performers: vec![
                AdultPerson {
                    provider_id: "dmm".into(),
                    remote_id: "performer-a".into(),
                    name: "Synthetic Person".into(),
                    aliases: vec!["Synthetic Alias".into()],
                },
                AdultPerson {
                    provider_id: "dmm".into(),
                    remote_id: "performer-b".into(),
                    name: "Synthetic Person".into(),
                    aliases: vec![],
                },
            ],
            directors: vec![],
            artwork: vec![],
        }
    }
    async fn setup() -> Setup {
        let dir = crate::test_support::tempdir().unwrap();
        let repo = LibraryRepository::open(&dir.path().join("library.sqlite"))
            .await
            .unwrap();
        let engine = NetworkEngine::for_loopback(2);
        let providers = ProviderService::with_engine(repo.clone(), engine)
            .await
            .unwrap();
        let hub = AdultProviderHub::new(providers.engine(), Arc::new(CredentialStore::new()));
        let service = AdultScraperService::new(
            repo.clone(),
            providers.clone(),
            Arc::new(hub),
            providers.engine(),
            dir.path().join("adult-cache"),
        );
        let now = Utc::now();
        let media = MediaItem {
            id: Uuid::new_v4(),
            kind: MediaKind::AdultVideo,
            title: "Local synthetic title".into(),
            original_title: None,
            year: None,
            created_at: now,
            updated_at: now,
        };
        repo.save_media(&media).await.unwrap();
        let root = LibraryRoot {
            id: Uuid::new_v4(),
            name: "Synthetic Adult Root".into(),
            path: dir.path().to_string_lossy().into_owned(),
            path_style: crate::test_support::native_path_style(),
            kind: Some(MediaKind::AdultVideo),
            enabled: true,
            created_at: now,
            updated_at: now,
        };
        repo.save_root(&root).await.unwrap();
        std::fs::write(
            dir.path().join("SYN-001-CD1.mkv"),
            b"synthetic source must remain unchanged",
        )
        .unwrap();
        repo.save_file(&MediaFile {
            id: Uuid::new_v4(),
            media_item_id: media.id,
            root_id: root.id,
            relative_path: "SYN-001-CD1.mkv".into(),
            size_bytes: 38,
            modified_at: None,
            part: Some(1),
        })
        .await
        .unwrap();
        let run = Uuid::new_v4();
        let job = ProviderJob {
            id: run,
            provider_id: "dmm".into(),
            media_item_id: None,
            media_kind: Some(MediaKind::AdultVideo),
            file_identity: None,
            region: None,
            correlation_id: run,
            operation: "search".into(),
            query: query().catalog_number,
            language: "en-US".into(),
            config: ProviderConfig {
                enabled: true,
                interval_ms: 0,
                timeout_ms: 4000,
                max_retries: 0,
                ..Default::default()
            },
            created_at: now,
        };
        let search = AdultSearchPreview {
            schema_version: 1,
            run_id: run,
            candidates: vec![match_adult_candidate(&query(), candidate(), &[])],
            failures: vec![],
        };
        sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES (?,'search_providers','waiting_for_user',?,?,?)").bind(run.to_string()).bind(r#"{"adult":true,"synthetic":true}"#).bind(now.timestamp_millis()).bind(now.timestamp_millis()).execute(repo.pool()).await.unwrap();
        sqlx::query("INSERT INTO provider_sessions(task_id,provider_id,job_json) VALUES (?,'adult_scraper',?)").bind(run.to_string()).bind(json(&job).unwrap()).execute(repo.pool()).await.unwrap();
        sqlx::query("INSERT INTO adult_scrape_sessions(task_id,query_json,jobs_json,search_json,created_at) VALUES (?,?,?,?,?)").bind(run.to_string()).bind(json(&query()).unwrap()).bind(json(&BTreeMap::from([("dmm".to_string(),job)])).unwrap()).bind(json(&search).unwrap()).bind(now.timestamp_millis()).execute(repo.pool()).await.unwrap();
        sqlx::query(
            "INSERT INTO adult_scrape_details(task_id,candidate_index,detail_json) VALUES (?,0,?)",
        )
        .bind(run.to_string())
        .bind(json(&detail()).unwrap())
        .execute(repo.pool())
        .await
        .unwrap();
        Setup {
            _dir: dir,
            repo,
            service,
            providers,
            media,
            root,
            run,
        }
    }
    async fn automatic_context(
        s: &Setup,
        access: crate::media_output::AdultOutputAccess,
    ) -> crate::scraper::automatic::AutomaticContext {
        let task_id = Uuid::new_v4();
        let now = Utc::now().timestamp_millis();
        sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES (?,'scan_library','running','{}',?,?)")
            .bind(task_id.to_string()).bind(now).bind(now).execute(s.repo.pool()).await.unwrap();
        sqlx::query("INSERT INTO scan_sessions(task_id,root_id,root_snapshot_json,mode,phase) VALUES(?,?,?,'automatic','metadata')")
            .bind(task_id.to_string()).bind(s.root.id.to_string()).bind(serde_json::json!({"root":s.root,"exclude_patterns":[]}).to_string()).execute(s.repo.pool()).await.unwrap();
        let file = s.repo.list_files(s.media.id).await.unwrap().remove(0);
        let normalized: String =
            sqlx::query_scalar("SELECT normalized_relative_path FROM media_files WHERE id=?")
                .bind(file.id.to_string())
                .fetch_one(s.repo.pool())
                .await
                .unwrap();
        sqlx::query("INSERT INTO scan_candidates(id,task_id,root_id,kind,relative_path,normalized_relative_path,payload_json,status,existing_media_id) VALUES(?,?,?,'adult_video',?,?,'{}','imported',?)")
            .bind(Uuid::new_v4().to_string()).bind(task_id.to_string()).bind(s.root.id.to_string()).bind(&file.relative_path).bind(&normalized).bind(s.media.id.to_string()).execute(s.repo.pool()).await.unwrap();
        sqlx::query("INSERT INTO library_refresh_items(task_id,media_item_id,state,updated_at) VALUES(?,?,'pending',?)")
            .bind(task_id.to_string()).bind(s.media.id.to_string()).bind(now).execute(s.repo.pool()).await.unwrap();
        sqlx::query("INSERT INTO metadata_values(id,media_item_id,field_key,locale,value_json,provider_id,is_user_modified,is_locked) VALUES(?,?,'catalog_number','','\"SYN-001\"','local',0,0)")
            .bind(Uuid::new_v4().to_string()).bind(s.media.id.to_string()).execute(s.repo.pool()).await.unwrap();
        crate::scraper::automatic::AutomaticContext {
            root: s.root.clone(),
            exclusions: vec![],
            task_id,
            cancel: CancellationToken::new(),
            adult_access: Some(access),
        }
    }
    async fn automatic_mock(
        s: &mut Setup,
        slow_detail: bool,
    ) -> (
        tokio::task::JoinHandle<()>,
        Arc<std::sync::atomic::AtomicBool>,
        Arc<tokio::sync::Notify>,
    ) {
        use crate::provider_auth::{AuthSaveInput, ProviderAuthService};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let started = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let detail_started = started.clone();
        let response_gate = Arc::new(tokio::sync::Notify::new());
        let release_detail = response_gate.clone();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut bytes = vec![0; 32768];
                let count = socket.read(&mut bytes).await.unwrap();
                if String::from_utf8_lossy(&bytes[..count]).contains("cid=") {
                    detail_started.store(true, std::sync::atomic::Ordering::Release);
                    if slow_detail {
                        release_detail.notified().await;
                    }
                }
                let body=serde_json::json!({"result":{"status":200,"items":[{"content_id":"syn00001","product_id":"SYN-001","title":"Synthetic automatic work","date":"2026-01-01","volume":"90","iteminfo":{"maker":[{"id":1,"name":"Synthetic Studio"}]}}]}}).to_string();
                let response=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });
        for descriptor in AdultProviderHub::descriptors() {
            s.providers
                .register_adult_descriptor(descriptor)
                .await
                .unwrap();
        }
        s.providers
            .update_config(
                "dmm",
                &ProviderConfig {
                    enabled: true,
                    interval_ms: 0,
                    max_retries: 0,
                    timeout_ms: 3000,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        let credentials = Arc::new(CredentialStore::new());
        let auth = ProviderAuthService::new(s.repo.clone(), credentials.clone());
        auth.save(AuthSaveInput {
            provider_id: "dmm".into(),
            method: "affiliate".into(),
            storage: "session_only".into(),
            secret: r#"{"api_id":"synthetic-api","affiliate_id":"synthetic-affiliate"}"#.into(),
        })
        .await
        .unwrap();
        let hub = AdultProviderHub::new(s.providers.engine(), credentials)
            .with_auth_service(auth.clone())
            .with_mock_endpoints(&format!("{base}/dmm"), &format!("{base}/wikidata"));
        s.service = AdultScraperService::new(
            s.repo.clone(),
            s.providers.clone(),
            Arc::new(hub),
            s.providers.engine(),
            s._dir.path().join("adult-cache"),
        )
        .with_auth_service(auth);
        (server, started, response_gate)
    }
    #[tokio::test]
    async fn automatic_context_uses_stored_normalized_file_identity() {
        let s = setup().await;
        // Reproduce Windows' case-folded database identity on every host while
        // retaining the source filename's original spelling.
        let file = s.repo.list_files(s.media.id).await.unwrap().remove(0);
        let normalized = mediaatlas_domain::normalize_relative_path(
            &file.relative_path,
            mediaatlas_domain::PathStyle::Windows,
        )
        .unwrap();
        assert_ne!(normalized, file.relative_path);
        sqlx::query("UPDATE media_files SET normalized_relative_path=? WHERE id=?")
            .bind(&normalized)
            .bind(file.id.to_string())
            .execute(s.repo.pool())
            .await
            .unwrap();
        let context = automatic_context(&s, crate::media_output::AdultOutputAccess::new()).await;
        let mut tx = s.repo.pool().begin().await.unwrap();
        context.validate_tx(&mut tx, s.media.id).await.unwrap();
        tx.rollback().await.unwrap();
        // The guard must continue rejecting a candidate for a different file.
        sqlx::query("UPDATE scan_candidates SET normalized_relative_path='different.mkv' WHERE task_id=?")
            .bind(context.task_id.to_string()).execute(s.repo.pool()).await.unwrap();
        let mut tx = s.repo.pool().begin().await.unwrap();
        assert!(matches!(
            context.validate_tx(&mut tx, s.media.id).await,
            Err(RepositoryError::Conflict)
        ));
        tx.rollback().await.unwrap();
    }
    #[tokio::test]
    async fn automatic_adult_catalog_enrichment_preserves_fields_and_never_writes_sidecars() {
        let mut s = setup().await;
        let (server, _, _) = automatic_mock(&mut s, false).await;
        let access = crate::media_output::AdultOutputAccess::new();
        let context = automatic_context(&s, access.clone()).await;
        let protected = local_value(
            s.media.id,
            "title",
            serde_json::json!("Protected local title"),
        );
        s.repo
            .save_metadata_value(&protected, MetadataWriteMode::User)
            .await
            .unwrap();
        let original = std::fs::read(s._dir.path().join("SYN-001-CD1.mkv")).unwrap();
        let service = s.service.clone().with_access(access);
        let outcome = service
            .enrich_automatic(s.media.id, &context)
            .await
            .unwrap();
        assert_eq!(outcome.state, "matched");
        let values = s.repo.list_metadata_values(s.media.id).await.unwrap();
        assert_eq!(
            values
                .iter()
                .find(|v| v.field_key == "title" && v.is_user_modified)
                .unwrap()
                .value,
            serde_json::json!("Protected local title")
        );
        assert!(service.media_detail(s.media.id).await.unwrap().is_some());
        assert_eq!(
            std::fs::read(s._dir.path().join("SYN-001-CD1.mkv")).unwrap(),
            original
        );
        assert!(!s._dir.path().join("SYN-001-CD1.nfo").exists());
        let ordinary_sources: i64 =
            sqlx::query_scalar("SELECT count(*) FROM video_metadata_sources")
                .fetch_one(s.repo.pool())
                .await
                .unwrap();
        assert_eq!(ordinary_sources, 0);
        let ordinary = s.providers.snapshot().await.unwrap();
        assert!(ordinary
            .tasks
            .iter()
            .all(|task| Some(task.id) != outcome.run_id));
        server.abort();
    }
    #[tokio::test]
    async fn automatic_adult_cancel_or_revocation_during_detail_prevents_publication() {
        for revoke in [false, true] {
            let mut s = setup().await;
            let (server, started, response_gate) = automatic_mock(&mut s, true).await;
            let access = crate::media_output::AdultOutputAccess::new();
            let context = automatic_context(&s, access.clone()).await;
            let cancel = context.cancel.clone();
            let service = s.service.clone().with_access(access.clone());
            let id = s.media.id;
            let operation =
                tokio::spawn(async move { service.enrich_automatic(id, &context).await });
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                while !started.load(std::sync::atomic::Ordering::Acquire) {
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            if revoke {
                access.revoke();
            } else {
                cancel.cancel();
            }
            response_gate.notify_one();
            assert!(operation.await.unwrap().is_err());
            let count: i64 = sqlx::query_scalar("SELECT count(*) FROM adult_metadata_sources")
                .fetch_one(s.repo.pool())
                .await
                .unwrap();
            assert_eq!(count, 0);
            assert_eq!(
                s.repo.get_media(id).await.unwrap().unwrap().title,
                s.media.title
            );
            server.abort();
        }
    }
    async fn settle_adult_scan(
        scans: &crate::scan_service::ScanService,
        id: Uuid,
    ) -> mediaatlas_application::library::ScanTaskView {
        tokio::time::timeout(std::time::Duration::from_secs(8), async {
            loop {
                let task = scans
                    .list_tasks(true)
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|t| t.id == id)
                    .unwrap();
                if !matches!(task.state, TaskState::Queued | TaskState::Running) {
                    return task;
                }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn adult_directory_refresh_indexes_and_identifies_without_manual_import() {
        let mut s = setup().await;
        let (server, _, _) = automatic_mock(&mut s, false).await;
        let access = crate::media_output::AdultOutputAccess::new();
        let scans = crate::scan_service::ScanService::new(s.repo.clone());
        let original = std::fs::read(s._dir.path().join("SYN-001-CD1.mkv")).unwrap();
        let task = scans
            .start_automatic_adult(s.root.id, s.service.clone(), access.clone())
            .await
            .unwrap();
        let finished = settle_adult_scan(&scans, task.id).await;
        assert_eq!(finished.state, TaskState::Succeeded);
        assert_eq!(finished.imported_count, 1);
        assert_eq!(finished.matched_count, 1);
        assert!(scans.list_tasks(false).await.unwrap().is_empty());
        let ordinary = crate::browse::BrowseService::new(
            s.repo.clone(),
            s._dir.path().join("ordinary-thumbs"),
        );
        assert!(ordinary
            .list_media(&Default::default())
            .await
            .unwrap()
            .items
            .is_empty());
        let adult =
            crate::browse::BrowseService::new(s.repo.clone(), s._dir.path().join("adult-thumbs"))
                .with_adult_access(access, false);
        assert_eq!(
            adult
                .list_media(&mediaatlas_application::library::BrowseQuery {
                    include_adult: true,
                    kind: Some(MediaKind::AdultVideo),
                    ..Default::default()
                })
                .await
                .unwrap()
                .items
                .len(),
            1
        );
        assert_eq!(
            std::fs::read(s._dir.path().join("SYN-001-CD1.mkv")).unwrap(),
            original
        );
        assert!(!s._dir.path().join("SYN-001-CD1.nfo").exists());
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM video_metadata_sources")
                .fetch_one(s.repo.pool())
                .await
                .unwrap(),
            0
        );
        server.abort();
    }
    #[tokio::test]
    async fn adult_directory_refresh_without_credentials_keeps_local_media_private_and_visible() {
        let s = setup().await;
        for descriptor in AdultProviderHub::descriptors() {
            s.providers
                .register_adult_descriptor(descriptor)
                .await
                .unwrap();
        }
        s.providers.set_enabled("dmm", true).await.unwrap();
        let access = crate::media_output::AdultOutputAccess::new();
        let scans = crate::scan_service::ScanService::new(s.repo.clone());
        let task = scans
            .start_automatic_adult(s.root.id, s.service.clone(), access.clone())
            .await
            .unwrap();
        let finished = settle_adult_scan(&scans, task.id).await;
        assert_eq!(finished.state, TaskState::WaitingForUser);
        assert_eq!(finished.review_count, 1);
        assert_eq!(finished.imported_count, 1);
        let ordinary = crate::browse::BrowseService::new(
            s.repo.clone(),
            s._dir.path().join("ordinary-thumbs"),
        );
        assert!(ordinary
            .list_media(&Default::default())
            .await
            .unwrap()
            .items
            .is_empty());
        let adult =
            crate::browse::BrowseService::new(s.repo.clone(), s._dir.path().join("adult-thumbs"))
                .with_adult_access(access.clone(), false);
        let page = adult
            .list_media(&mediaatlas_application::library::BrowseQuery {
                include_adult: true,
                kind: Some(MediaKind::AdultVideo),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(page.items.len(), 1);
        assert!(page.items[0]
            .health
            .contains(&"metadata_unconfigured".into()));
        access.revoke();
        assert!(scans
            .start_automatic_adult(s.root.id, s.service.clone(), access)
            .await
            .is_err());
    }
    #[tokio::test]
    async fn locking_adult_directory_refresh_cancels_late_network_work() {
        let mut s = setup().await;
        let (server, started, response_gate) = automatic_mock(&mut s, true).await;
        let access = crate::media_output::AdultOutputAccess::new();
        let scans = crate::scan_service::ScanService::new(s.repo.clone());
        let task = scans
            .start_automatic_adult(s.root.id, s.service.clone(), access.clone())
            .await
            .unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            while !started.load(std::sync::atomic::Ordering::Acquire) {
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        access.revoke();
        let finished = settle_adult_scan(&scans, task.id).await;
        response_gate.notify_one();
        assert_eq!(finished.state, TaskState::Cancelled);
        assert_eq!(
            sqlx::query_scalar::<_, i64>("SELECT count(*) FROM adult_metadata_sources")
                .fetch_one(s.repo.pool())
                .await
                .unwrap(),
            0
        );
        assert!(scans.list_tasks(false).await.unwrap().is_empty());
        server.abort();
    }
    fn selection(s: &Setup) -> AdultMergeRequest {
        AdultMergeRequest {
            run_id: s.run,
            candidate_index: 0,
            media_item_id: s.media.id,
            selected_fields: vec![
                "title".into(),
                "catalog_numbers".into(),
                "studio".into(),
                "label".into(),
                "series".into(),
                "performers".into(),
            ],
            selected_artwork: vec![],
        }
    }
    fn local_value(id: Uuid, key: &str, value: serde_json::Value) -> MetadataValue {
        MetadataValue {
            id: Uuid::new_v4(),
            media_item_id: id,
            field_key: key.into(),
            locale: Some("en-US".into()),
            value,
            provider_id: None,
            source_url: None,
            fetched_at: None,
            confidence: None,
            is_user_modified: true,
            is_locked: true,
        }
    }
    #[tokio::test]
    async fn adult_merge_requires_preview_and_preserves_source_people_and_provenance() {
        let s = setup().await;
        assert!(s.service.media_detail(s.media.id).await.unwrap().is_none());
        let preview = s.service.preview(selection(&s)).await.unwrap();
        assert_eq!(
            s.repo.get_media(s.media.id).await.unwrap().unwrap().title,
            s.media.title
        );
        let result = s.service.confirm(preview.preview_id).await.unwrap();
        assert_eq!(result.applied_fields, 6);
        let saved = s.service.media_detail(s.media.id).await.unwrap().unwrap();
        assert_eq!(saved.performers.len(), 2);
        assert_ne!(saved.performers[0].remote_id, saved.performers[1].remote_id);
        assert_eq!(saved.performers[0].name, saved.performers[1].name);
        assert!(
            s.service
                .confirm(preview.preview_id)
                .await
                .unwrap()
                .already_applied
        );
        let values = s.repo.list_metadata_values(s.media.id).await.unwrap();
        assert!(values
            .iter()
            .all(|v| v.provider_id.as_deref() == Some("dmm")));
        assert_eq!(
            std::fs::read(s._dir.path().join("SYN-001-CD1.mkv")).unwrap(),
            b"synthetic source must remain unchanged"
        );
        assert!(crate::scraper::ScraperService::new(
            s.repo.clone(),
            s.providers.clone(),
            Arc::new(crate::video_providers::VideoProviderHub::new(
                s.providers.engine(),
                Arc::new(CredentialStore::new())
            )),
            s.providers.engine(),
            s._dir.path().join("ordinary-cache")
        )
        .media_detail(s.media.id)
        .await
        .is_err());
    }
    #[tokio::test]
    async fn locked_fields_and_concurrent_manual_edit_reject_atomic_confirmation() {
        let s = setup().await;
        let preview = s.service.preview(selection(&s)).await.unwrap();
        s.repo
            .save_metadata_value(
                &local_value(s.media.id, "title", serde_json::json!("Manual title")),
                MetadataWriteMode::User,
            )
            .await
            .unwrap();
        assert!(matches!(
            s.service.confirm(preview.preview_id).await,
            Err(RepositoryError::ProtectedMetadata)
        ));
        assert_eq!(
            s.repo.get_media(s.media.id).await.unwrap().unwrap().title,
            s.media.title
        );
        assert!(s.service.media_detail(s.media.id).await.unwrap().is_none());
        let second = s.service.preview(selection(&s)).await.unwrap();
        let title = second.fields.iter().find(|f| f.key == "title").unwrap();
        assert!(title.protected);
        assert!(!title.selected);
        s.service.confirm(second.preview_id).await.unwrap();
        assert_eq!(
            s.repo.get_media(s.media.id).await.unwrap().unwrap().title,
            s.media.title
        );
    }
    #[tokio::test]
    async fn root_relocation_disable_and_same_timestamp_canonical_edit_invalidate_preview() {
        let s = setup().await;
        let preview = s.service.preview(selection(&s)).await.unwrap();
        sqlx::query("UPDATE library_roots SET enabled=0 WHERE id=?")
            .bind(s.root.id.to_string())
            .execute(s.repo.pool())
            .await
            .unwrap();
        assert!(s.service.confirm(preview.preview_id).await.is_err());
        sqlx::query("UPDATE library_roots SET enabled=1 WHERE id=?")
            .bind(s.root.id.to_string())
            .execute(s.repo.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE media_items SET title='Concurrent synthetic edit' WHERE id=?")
            .bind(s.media.id.to_string())
            .execute(s.repo.pool())
            .await
            .unwrap();
        assert!(matches!(
            s.service.confirm(preview.preview_id).await,
            Err(RepositoryError::Conflict)
        ));
        assert!(s.service.media_detail(s.media.id).await.unwrap().is_none());
    }
    #[tokio::test]
    async fn cancelled_task_cannot_confirm_or_retrieve_old_results() {
        let s = setup().await;
        let preview = s.service.preview(selection(&s)).await.unwrap();
        assert!(s.providers.cancel(s.run).await.unwrap());
        assert!(s.service.confirm(preview.preview_id).await.is_err());
        assert!(s.service.search_preview(s.run).await.is_err());
        assert!(s.service.detail(s.run, 0).await.is_err());
        assert!(s
            .repo
            .list_metadata_values(s.media.id)
            .await
            .unwrap()
            .is_empty());
    }
    #[tokio::test]
    async fn ordinary_or_mixed_root_is_never_an_adult_merge_destination() {
        let s = setup().await;
        let mut input = selection(&s);
        let ordinary = MediaItem {
            id: Uuid::new_v4(),
            kind: MediaKind::Movie,
            ..s.media.clone()
        };
        s.repo.save_media(&ordinary).await.unwrap();
        input.media_item_id = ordinary.id;
        assert!(s.service.preview(input).await.is_err());
        sqlx::query("UPDATE library_roots SET kind=NULL WHERE id=?")
            .bind(s.root.id.to_string())
            .execute(s.repo.pool())
            .await
            .unwrap();
        assert!(s.service.preview(selection(&s)).await.is_err());
    }
    #[tokio::test]
    async fn immutable_sessions_previews_details_and_catalog_conflict_cannot_be_rewritten() {
        let s = setup().await;
        let preview = s.service.preview(selection(&s)).await.unwrap();
        for statement in [
            "UPDATE adult_scrape_sessions SET jobs_json='{}' WHERE task_id=?",
            "UPDATE adult_scrape_sessions SET search_json='{}' WHERE task_id=?",
            "UPDATE adult_scrape_details SET detail_json='{}' WHERE task_id=?",
        ] {
            assert!(sqlx::query(statement)
                .bind(s.run.to_string())
                .execute(s.repo.pool())
                .await
                .is_err());
        }
        assert!(
            sqlx::query("UPDATE adult_merge_previews SET detail_json='{}' WHERE id=?")
                .bind(preview.preview_id.to_string())
                .execute(s.repo.pool())
                .await
                .is_err()
        );
        sqlx::query("INSERT INTO external_ids(id,media_item_id,provider_id,namespace,external_id,is_primary,created_at) VALUES (?,?,?,'synthetic.issuer','SYN-999',1,?)").bind(Uuid::new_v4().to_string()).bind(s.media.id.to_string()).bind("dmm").bind(Utc::now().timestamp_millis()).execute(s.repo.pool()).await.unwrap();
        assert!(matches!(
            s.service.confirm(preview.preview_id).await,
            Err(RepositoryError::Conflict)
        ));
        assert!(s
            .repo
            .list_metadata_values(s.media.id)
            .await
            .unwrap()
            .is_empty());
    }
    #[test]
    fn catalog_studio_namespace_and_part_rules_do_not_use_movie_scores() {
        let q = query();
        let exact = match_adult_candidate(&q, candidate(), &[]);
        assert!(exact.reasons.contains(&"catalog_exact".into()));
        assert!(!exact.needs_review);
        let mut conflict = candidate();
        conflict.studio = Some("Different Synthetic Studio".into());
        let matched = match_adult_candidate(&q, conflict.clone(), std::slice::from_ref(&exact));
        assert!(matched.needs_review);
        assert_eq!(matched.duplicate_of, None);
        assert!(validate_query_identity(&q, &conflict).is_err());
        let mut other = candidate();
        other.catalog_number.namespace = "other.issuer".into();
        assert!(match_adult_candidate(&q, other, &[exact]).needs_review);
        let mut part = candidate();
        part.catalog_number = normalize_catalog_number("SYN-001-CD2", "synthetic.issuer").unwrap();
        assert!(validate_query_identity(&q, &part).is_err());
        let mut wrong = detail();
        wrong.identity.catalog_number =
            normalize_catalog_number("SYN-999", "synthetic.issuer").unwrap();
        assert!(validate_detail(&candidate(), &wrong).is_err());
        let mut wrong = detail();
        wrong.identity.studio = Some("Different Studio".into());
        assert!(validate_detail(&candidate(), &wrong).is_err());
    }
    #[tokio::test]
    async fn revoked_grant_rejects_existing_sessions_previews_and_source_reads() {
        let s = setup().await;
        let grant = crate::media_output::AdultOutputAccess::new();
        let service = s.service.clone().with_access(grant.clone());
        let preview = service.preview(selection(&s)).await.unwrap();
        grant.revoke();
        assert!(service.confirm(preview.preview_id).await.is_err());
        assert!(service.search_preview(s.run).await.is_err());
        assert!(service.detail(s.run, 0).await.is_err());
        assert!(service.history().await.is_err());
        assert!(service.media_detail(s.media.id).await.is_err());
        assert!(s
            .repo
            .list_metadata_values(s.media.id)
            .await
            .unwrap()
            .is_empty());
    }
    #[tokio::test]
    async fn actual_mock_api_aggregation_failure_isolation_and_disable_keep_adult_lane_private() {
        use crate::provider_auth::{AuthSaveInput, ProviderAuthService};
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let mut s = setup().await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                let mut bytes = vec![0; 32768];
                let count = socket.read(&mut bytes).await.unwrap();
                let request = String::from_utf8_lossy(&bytes[..count]);
                let (status, body) = if request.starts_with("GET /dmm") {
                    ("200 OK",serde_json::json!({"result":{"status":200,"items":[{"content_id":"syn00001","product_id":"SYN-001","title":"Synthetic API work","date":"2026-01-01","volume":"90","iteminfo":{"maker":[{"id":1,"name":"Synthetic Studio"}],"actress":[{"id":10,"name":"Synthetic Performer"}]}}]}}).to_string())
                } else {
                    (
                        "403 Forbidden",
                        r#"{"error":"synthetic blocked supplement"}"#.into(),
                    )
                };
                let response=format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });
        for descriptor in AdultProviderHub::descriptors() {
            s.providers
                .register_adult_descriptor(descriptor)
                .await
                .unwrap();
        }
        for id in ["dmm", "wikidata"] {
            s.providers
                .update_config(
                    id,
                    &ProviderConfig {
                        enabled: true,
                        interval_ms: 0,
                        max_retries: 0,
                        timeout_ms: 4000,
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
        }
        let credentials = Arc::new(CredentialStore::new());
        let auth = ProviderAuthService::new(s.repo.clone(), credentials.clone());
        auth.save(AuthSaveInput {
            provider_id: "dmm".into(),
            method: "affiliate".into(),
            storage: "session_only".into(),
            secret:
                r#"{"api_id":"synthetic-api-value","affiliate_id":"synthetic-affiliate-value"}"#
                    .into(),
        })
        .await
        .unwrap();
        let hub = AdultProviderHub::new(s.providers.engine(), credentials)
            .with_auth_service(auth.clone())
            .with_mock_endpoints(&format!("{base}/dmm"), &format!("{base}/wikidata"));
        s.service = AdultScraperService::new(
            s.repo.clone(),
            s.providers.clone(),
            Arc::new(hub),
            s.providers.engine(),
            s._dir.path().join("adult-cache"),
        )
        .with_auth_service(auth);
        let request = AdultSearchRequest {
            query: AdultQuery {
                namespace: "dmm.product".into(),
                ..query()
            },
            providers: vec!["dmm".into(), "wikidata".into()],
        };
        let result = s.service.search(request.clone()).await.unwrap();
        assert_eq!(result.candidates.len(), 1);
        assert_eq!(result.failures.len(), 1);
        assert_eq!(result.failures[0].provider_id, "wikidata");
        let remote = s.service.detail(result.run_id, 0).await.unwrap();
        assert_eq!(remote.identity.catalog_number.normalized, "SYN-001");
        let preview = s
            .service
            .preview(AdultMergeRequest {
                run_id: result.run_id,
                candidate_index: 0,
                media_item_id: s.media.id,
                selected_fields: vec!["title".into(), "studio".into()],
                selected_artwork: vec![],
            })
            .await
            .unwrap();
        s.service.confirm(preview.preview_id).await.unwrap();
        assert_eq!(
            s.repo.get_media(s.media.id).await.unwrap().unwrap().title,
            "Synthetic API work"
        );
        let persisted: String =
            sqlx::query_scalar("SELECT jobs_json FROM adult_scrape_sessions WHERE task_id=?")
                .bind(result.run_id.to_string())
                .fetch_one(s.repo.pool())
                .await
                .unwrap();
        assert!(persisted.contains("dmm.auth.affiliate.session."));
        assert!(!persisted.contains("synthetic-api-value"));
        assert!(!persisted.contains("synthetic-affiliate-value"));
        let ordinary = s.providers.snapshot().await.unwrap();
        assert!(ordinary.tasks.iter().all(|task| task.id != result.run_id));
        assert!(ordinary
            .requests
            .iter()
            .all(|record| record.task_id != result.run_id));
        s.providers.set_enabled("wikidata", false).await.unwrap();
        let after = s.service.search(request).await.unwrap();
        assert_eq!(after.candidates.len(), 1);
        assert_eq!(after.failures[0].provider_id, "wikidata");
        assert_eq!(after.failures[0].failure, ProviderFailure::Disabled);
        assert!(s.providers.config("dmm").await.unwrap().enabled);
        server.abort();
    }
    #[tokio::test]
    async fn cancelling_or_locking_during_selected_image_download_cannot_commit_metadata() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for lock in [false, true] {
            let s = setup().await;
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let image_url = format!("http://{}/synthetic.png", listener.local_addr().unwrap());
            let started = Arc::new(tokio::sync::Notify::new());
            let signal = started.clone();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = vec![0; 4096];
                let _ = socket.read(&mut request).await.unwrap();
                signal.notify_one();
                tokio::time::sleep(std::time::Duration::from_millis(120)).await;
                let mut bytes = std::io::Cursor::new(Vec::new());
                image::DynamicImage::new_rgb8(2, 2)
                    .write_to(&mut bytes, image::ImageFormat::Png)
                    .unwrap();
                let body = bytes.into_inner();
                let header=format!("HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());
                let _ = socket.write_all(header.as_bytes()).await;
                let _ = socket.write_all(&body).await;
            });
            let mut fixture = detail();
            fixture.artwork = vec![ArtworkCandidate {
                url: image_url,
                source_provider: "dmm".into(),
                kind: "cover".into(),
                language: None,
            }];
            sqlx::query("DELETE FROM adult_scrape_details WHERE task_id=?")
                .bind(s.run.to_string())
                .execute(s.repo.pool())
                .await
                .unwrap();
            sqlx::query("INSERT INTO adult_scrape_details(task_id,candidate_index,detail_json) VALUES (?,0,?)").bind(s.run.to_string()).bind(json(&fixture).unwrap()).execute(s.repo.pool()).await.unwrap();
            let grant = crate::media_output::AdultOutputAccess::new();
            let service = s.service.clone().with_access(grant.clone());
            let mut request = selection(&s);
            request.selected_artwork = vec![0];
            let preview = service.preview(request).await.unwrap();
            let execution = tokio::spawn(async move { service.confirm(preview.preview_id).await });
            tokio::time::timeout(std::time::Duration::from_secs(3), started.notified())
                .await
                .unwrap();
            if lock {
                grant.revoke();
            } else {
                assert!(s.providers.cancel(s.run).await.unwrap());
            }
            assert!(execution.await.unwrap().is_err());
            assert!(s
                .repo
                .list_metadata_values(s.media.id)
                .await
                .unwrap()
                .is_empty());
            assert!(s.repo.list_artworks(s.media.id).await.unwrap().is_empty());
            assert_eq!(
                s.repo.get_media(s.media.id).await.unwrap().unwrap().title,
                s.media.title
            );
            server.abort();
        }
    }
}
