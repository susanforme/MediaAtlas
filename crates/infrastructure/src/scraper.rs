//! Persistent, typed scraping with explicit review and protected metadata writes.
pub(crate) mod automatic;
use crate::{
    library::{LibraryRepository, RepositoryError},
    network::{CancellationToken, NetworkEngine},
    providers::ProviderService,
    video_providers::VideoProviderHub,
};
use chrono::Utc;
use mediaatlas_domain::*;
use serde::{Deserialize, Serialize};
use sqlx::Row;
use std::{
    collections::{BTreeMap, HashSet},
    path::PathBuf,
    sync::Arc,
};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone)]
pub struct ScraperService {
    repo: LibraryRepository,
    providers: ProviderService,
    hub: Arc<VideoProviderHub>,
    engine: Arc<NetworkEngine>,
    cache: PathBuf,
    gate: Arc<Mutex<()>>,
    auth: Option<crate::provider_auth::ProviderAuthService>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtworkPreview {
    pub thumbnail: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScraperHistoryEntry {
    pub id: Uuid,
    pub query: VideoQuery,
    pub state: String,
    pub created_at: chrono::DateTime<Utc>,
    pub candidate_count: usize,
}
impl ScraperService {
    pub fn new(
        repo: LibraryRepository,
        providers: ProviderService,
        hub: Arc<VideoProviderHub>,
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
        }
    }
    pub fn with_auth_service(mut self, auth: crate::provider_auth::ProviderAuthService) -> Self {
        self.auth = Some(auth);
        self
    }
    pub async fn initialize(&self) -> Result<(), RepositoryError> {
        sqlx::query("UPDATE video_merge_previews SET state='interrupted' WHERE state='running'")
            .execute(self.repo.pool())
            .await?;
        Ok(())
    }
    pub async fn search(
        &self,
        request: ScraperSearchRequest,
    ) -> Result<SearchPreview, RepositoryError> {
        self.search_cancellable(request, CancellationToken::new())
            .await
    }
    async fn search_cancellable(
        &self,
        request: ScraperSearchRequest,
        cancellation: CancellationToken,
    ) -> Result<SearchPreview, RepositoryError> {
        validate_query(&request.query)?;
        if request.providers.is_empty() || request.providers.len() > 3 {
            return Err(RepositoryError::InvalidInput);
        }
        let mut seen = HashSet::new();
        let mut jobs = BTreeMap::new();
        let id = Uuid::new_v4();
        let now = Utc::now();
        for provider in &request.providers {
            if !seen.insert(provider)
                || !VideoProviderHub::descriptors()
                    .iter()
                    .any(|d| &d.id == provider)
            {
                return Err(RepositoryError::InvalidInput);
            }
            let mut config = self.providers.config(provider).await?;
            if let Some(auth) = &self.auth {
                config.credential_ref = auth.snapshot_ref(provider).await?;
            }
            let (title, _, _, language) = request.query.parts();
            jobs.insert(
                provider.clone(),
                ProviderJob {
                    id,
                    provider_id: provider.clone(),
                    media_item_id: None,
                    media_kind: Some(query_kind(&request.query)),
                    file_identity: None,
                    region: None,
                    correlation_id: id,
                    operation: "search".into(),
                    query: title.into(),
                    language: language.into(),
                    config,
                    created_at: now,
                },
            );
        }
        let first = jobs
            .values()
            .next()
            .ok_or(RepositoryError::InvalidInput)?
            .clone();
        let mut tx = self.repo.pool().begin().await?;
        sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES (?,'search_providers','running',?,?,?)").bind(id.to_string()).bind(json(&request)?).bind(now.timestamp_millis()).bind(now.timestamp_millis()).execute(&mut *tx).await?;
        sqlx::query(
            "INSERT INTO provider_sessions(task_id,provider_id,job_json) VALUES (?,'scraper',?)",
        )
        .bind(id.to_string())
        .bind(json(&first)?)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO video_scrape_sessions(task_id,query_json,jobs_json,created_at) VALUES (?,?,?,?)").bind(id.to_string()).bind(json(&request.query)?).bind(json(&jobs)?).bind(now.timestamp_millis()).execute(&mut *tx).await?;
        for job in jobs.values() {
            sqlx::query("INSERT INTO scrape_runs(id,task_id,provider_id,config_snapshot,state,started_at) VALUES (?,?,?,?,'running',?)").bind(Uuid::new_v4().to_string()).bind(id.to_string()).bind(&job.provider_id).bind(json(&job.config)?).bind(now.timestamp_millis()).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        let cancel = self.providers.attach_cancellation(id, cancellation).await;
        let mut result = SearchPreview {
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
                let result = hub.search(&job.provider_id, &query, &job, &cancel).await;
                (job.provider_id, result)
            });
        }
        while let Some(outcome) = pending.join_next().await {
            let (provider_id, fetched) = outcome.map_err(|_| RepositoryError::InvalidInput)?;
            match fetched {
                Ok(candidates) => {
                    for candidate in candidates {
                        if candidate.kind() != request.query.kind() {
                            result.failures.push(ProviderSearchFailure {
                                provider_id: provider_id.clone(),
                                failure: ProviderFailure::IdentityMismatch,
                            });
                            continue;
                        }
                        result.candidates.push(match_candidate(
                            &request.query,
                            candidate,
                            &result.candidates,
                        ));
                    }
                }
                Err(failure) => result.failures.push(ProviderSearchFailure {
                    provider_id,
                    failure,
                }),
            }
        }
        self.persist_records(id).await?;
        self.providers.detach_cancellation(id).await;
        let mut tx = self.repo.pool().begin().await?;
        let updated=sqlx::query("UPDATE scrape_tasks SET state='waiting_for_user',updated_at=MAX(updated_at,?) WHERE id=? AND state='running'").bind(Utc::now().timestamp_millis()).bind(id.to_string()).execute(&mut *tx).await?;
        if updated.rows_affected() != 1 {
            return Err(RepositoryError::Conflict);
        }
        sqlx::query("UPDATE video_scrape_sessions SET search_json=? WHERE task_id=?")
            .bind(json(&result)?)
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?;
        for job in jobs.values() {
            let failure = result
                .failures
                .iter()
                .find(|f| f.provider_id == job.provider_id);
            sqlx::query("UPDATE scrape_runs SET state=?,finished_at=MAX(started_at,?) WHERE task_id=? AND provider_id=?").bind(if failure.is_some(){"failed"}else{"succeeded"}).bind(Utc::now().timestamp_millis()).bind(id.to_string()).bind(&job.provider_id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok(result)
    }
    async fn session(
        &self,
        id: Uuid,
    ) -> Result<(VideoQuery, BTreeMap<String, ProviderJob>, SearchPreview), RepositoryError> {
        let row=sqlx::query("SELECT v.query_json,v.jobs_json,v.search_json,t.state FROM video_scrape_sessions v JOIN scrape_tasks t ON t.id=v.task_id WHERE task_id=?").bind(id.to_string()).fetch_optional(self.repo.pool()).await?.ok_or(RepositoryError::NotFound)?;
        let state: String = row.try_get("state")?;
        if !matches!(state.as_str(), "waiting_for_user" | "succeeded") {
            return Err(RepositoryError::Conflict);
        }
        Ok((
            decode(&row.try_get::<String, _>("query_json")?)?,
            decode(&row.try_get::<String, _>("jobs_json")?)?,
            decode(
                &row.try_get::<Option<String>, _>("search_json")?
                    .ok_or(RepositoryError::Conflict)?,
            )?,
        ))
    }
    pub async fn detail(
        &self,
        run_id: Uuid,
        index: usize,
    ) -> Result<VideoMetadata, RepositoryError> {
        self.detail_cancellable(run_id, index, CancellationToken::new())
            .await
    }
    async fn detail_cancellable(
        &self,
        run_id: Uuid,
        index: usize,
        cancellation: CancellationToken,
    ) -> Result<VideoMetadata, RepositoryError> {
        let (query, jobs, search) = self.session(run_id).await?;
        let candidate = &search
            .candidates
            .get(index)
            .ok_or(RepositoryError::InvalidInput)?
            .candidate;
        if let Some(data) = sqlx::query_scalar::<_, String>(
            "SELECT detail_json FROM video_scrape_details WHERE task_id=? AND candidate_index=?",
        )
        .bind(run_id.to_string())
        .bind(index as i64)
        .fetch_optional(self.repo.pool())
        .await?
        {
            return decode(&data);
        }
        let mut job = jobs
            .get(&candidate.data().provider_id)
            .ok_or(RepositoryError::CorruptData)?
            .clone();
        job.operation = "detail".into();
        let cancel = self
            .providers
            .attach_cancellation(run_id, cancellation)
            .await;
        if self.session(run_id).await.is_err() {
            self.providers.detach_cancellation(run_id).await;
            return Err(RepositoryError::Conflict);
        }
        let detail = tokio::time::timeout(
            std::time::Duration::from_millis(job.config.timeout_ms.max(1)),
            self.hub.detail(candidate, &job, &cancel),
        )
        .await
        .unwrap_or(Err(ProviderFailure::Timeout));
        self.persist_records(run_id).await?;
        self.providers.detach_cancellation(run_id).await;
        let detail = detail.map_err(RepositoryError::Provider)?;
        if detail.identity().remote_id != candidate.data().remote_id
            || detail.identity().provider_id != candidate.data().provider_id
            || metadata_kind(&detail) != query_kind(&query)
        {
            return Err(RepositoryError::InvalidInput);
        }
        validate_fields(detail.fields())?;
        self.session(run_id).await?;
        sqlx::query("INSERT OR IGNORE INTO video_scrape_details(task_id,candidate_index,detail_json) VALUES (?,?,?)").bind(run_id.to_string()).bind(index as i64).bind(json(&detail)?).execute(self.repo.pool()).await?;
        let persisted: String = sqlx::query_scalar(
            "SELECT detail_json FROM video_scrape_details WHERE task_id=? AND candidate_index=?",
        )
        .bind(run_id.to_string())
        .bind(index as i64)
        .fetch_one(self.repo.pool())
        .await?;
        decode(&persisted)
    }
    pub async fn artwork_preview(
        &self,
        run_id: Uuid,
        index: usize,
        artwork_index: usize,
    ) -> Result<ArtworkPreview, RepositoryError> {
        use base64::Engine;
        let detail = self.detail(run_id, index).await?;
        let (_, jobs, _) = self.session(run_id).await?;
        let mut job = jobs
            .get(&detail.identity().provider_id)
            .ok_or(RepositoryError::CorruptData)?
            .clone();
        job.operation = "artwork_preview".into();
        let art = detail
            .artwork()
            .get(artwork_index)
            .ok_or(RepositoryError::InvalidInput)?;
        let service =
            crate::media_output::MediaOutputService::new(self.repo.clone(), self.cache.clone());
        let cancel = self
            .providers
            .attach_cancellation(run_id, CancellationToken::new())
            .await;
        if self.session(run_id).await.is_err() {
            self.providers.detach_cancellation(run_id).await;
            return Err(RepositoryError::Conflict);
        }
        let cached = service
            .cache_artwork(&self.engine, &job, &art.url, &cancel)
            .await;
        self.persist_records(run_id).await?;
        self.providers.detach_cancellation(run_id).await;
        self.session(run_id).await?;
        let cached = cached.map_err(RepositoryError::Provider)?;
        let bytes = service.thumbnail_bytes(&cached)?;
        Ok(ArtworkPreview {
            thumbnail: format!(
                "data:image/png;base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            ),
        })
    }
    pub async fn preview(&self, request: MergeRequest) -> Result<MergePreview, RepositoryError> {
        let (_, _, search) = self.session(request.run_id).await?;
        search
            .candidates
            .get(request.candidate_index)
            .ok_or(RepositoryError::InvalidInput)?;
        let detail = self.detail(request.run_id, request.candidate_index).await?;
        let media = self
            .ordinary_media(request.media_item_id, metadata_kind(&detail))
            .await?;
        let old = self.repo.list_metadata_values(media.id).await?;
        let keys: HashSet<_> = request.selected_fields.iter().collect();
        if keys.len() != request.selected_fields.len()
            || !keys
                .iter()
                .all(|k| detail.fields().iter().filter(|f| &f.key == *k).count() == 1)
        {
            return Err(RepositoryError::InvalidInput);
        }
        let mut fields = vec![];
        for field in detail.fields() {
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
            let empty = empty_value(&field.value);
            fields.push(FieldDiff {
                key: field.key.clone(),
                language,
                old: previous.clone(),
                new: field.clone(),
                provider_id: detail.identity().provider_id.clone(),
                source_url: detail.identity().source_url.clone(),
                protected,
                needs_review: previous.as_ref().is_some_and(|v| v.value != field.value),
                selected: keys.contains(&field.key) && !protected && !empty,
            });
        }
        let mut artwork = vec![];
        let mut indices = HashSet::new();
        for i in &request.selected_artwork {
            if !indices.insert(i) {
                return Err(RepositoryError::InvalidInput);
            }
            artwork.push(
                detail
                    .artwork()
                    .get(*i)
                    .ok_or(RepositoryError::InvalidInput)?
                    .clone(),
            );
        }
        let preview = MergePreview {
            schema_version: 1,
            preview_id: Uuid::new_v4(),
            media_item_id: media.id,
            fields,
            artwork,
            confirmed: false,
        };
        sqlx::query("INSERT INTO video_merge_previews(id,task_id,media_item_id,preview_json,detail_json,selected_artwork,expected_media_json,state,created_at) VALUES (?,?,?,?,?,?,?,'planned',?)").bind(preview.preview_id.to_string()).bind(request.run_id.to_string()).bind(media.id.to_string()).bind(json(&preview)?).bind(json(&detail)?).bind(json(&request.selected_artwork)?).bind(json(&media)?).bind(Utc::now().timestamp_millis()).execute(self.repo.pool()).await?;
        Ok(preview)
    }
    pub async fn confirm(&self, id: Uuid) -> Result<MergeResult, RepositoryError> {
        self.confirm_inner(id, None).await
    }
    async fn confirm_inner(
        &self,
        id: Uuid,
        automatic: Option<&automatic::AutomaticContext>,
    ) -> Result<MergeResult, RepositoryError> {
        let _guard = self.gate.lock().await;
        let row = sqlx::query("SELECT * FROM video_merge_previews WHERE id=?")
            .bind(id.to_string())
            .fetch_optional(self.repo.pool())
            .await?
            .ok_or(RepositoryError::NotFound)?;
        let preview: MergePreview = decode(&row.try_get::<String, _>("preview_json")?)?;
        let detail: VideoMetadata = decode(&row.try_get::<String, _>("detail_json")?)?;
        if row.try_get::<String, _>("state")? == "succeeded" {
            let mut result: MergeResult = decode(&row.try_get::<String, _>("result_json")?)?;
            result.already_applied = true;
            return Ok(result);
        }
        let media = self
            .ordinary_media(preview.media_item_id, metadata_kind(&detail))
            .await?;
        let expected: MediaItem = decode(&row.try_get::<String, _>("expected_media_json")?)?;
        if media != expected {
            return Err(RepositoryError::Conflict);
        }
        let run_id = Uuid::parse_str(&row.try_get::<String, _>("task_id")?)
            .map_err(|_| RepositoryError::CorruptData)?;
        let (_, jobs, _) = self.session(run_id).await?;
        let mut job = jobs
            .get(&detail.identity().provider_id)
            .ok_or(RepositoryError::CorruptData)?
            .clone();
        job.operation = "artwork".into();
        // Download only the explicitly selected immutable gallery entries. No media/metadata writes on failure.
        let mut artworks = vec![];
        let cancel = self
            .providers
            .attach_cancellation(
                run_id,
                automatic.map(|a| a.cancel.clone()).unwrap_or_default(),
            )
            .await;
        if self.session(run_id).await.is_err() {
            self.providers.detach_cancellation(run_id).await;
            return Err(RepositoryError::Conflict);
        }
        for art in &preview.artwork {
            let cached =
                crate::media_output::MediaOutputService::new(self.repo.clone(), self.cache.clone())
                    .cache_artwork(&self.engine, &job, &art.url, &cancel)
                    .await;
            if let Err(failure) = cached {
                self.persist_records(run_id).await?;
                self.providers.detach_cancellation(run_id).await;
                return Err(RepositoryError::Provider(failure));
            }
            artworks.push((art.clone(), cached.map_err(RepositoryError::Provider)?));
        }
        self.persist_records(run_id).await?;
        self.providers.detach_cancellation(run_id).await;
        self.session(run_id).await?;
        let mut tx = self.repo.pool().begin().await?;
        if let Some(context) = automatic {
            context.validate_tx(&mut tx, media.id).await?;
        }
        let current=sqlx::query("SELECT kind,title,original_title,year,updated_at,adult_isolated FROM media_items WHERE id=?").bind(media.id.to_string()).fetch_one(&mut *tx).await?;
        if current.try_get::<String, _>("kind")? != metadata_kind(&detail).as_str()
            || current.try_get::<String, _>("title")? != expected.title
            || current.try_get::<Option<String>, _>("original_title")? != expected.original_title
            || current.try_get::<Option<i32>, _>("year")? != expected.year
            || current.try_get::<bool, _>("adult_isolated")?
            || current.try_get::<i64, _>("updated_at")? != expected.updated_at.timestamp_millis()
        {
            return Err(RepositoryError::Conflict);
        }
        let mut applied = 0;
        let mut title = None;
        let mut original = None;
        let mut year = None;
        for diff in preview.fields.iter().filter(|f| {
            f.selected
                || (detail.identity().provider_id == "wikipedia.html"
                    && ["attribution", "source_license"].contains(&f.key.as_str()))
        }) {
            let locale = diff.language.as_deref().unwrap_or("");
            let previous=sqlx::query("SELECT id,value_json,is_locked,is_user_modified FROM metadata_values WHERE media_item_id=? AND field_key=? AND locale=?").bind(media.id.to_string()).bind(&diff.key).bind(locale).fetch_optional(&mut *tx).await?;
            if let Some(ref prev) = previous {
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
                .map(|r| r.try_get::<String, _>("id"))
                .transpose()?
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            sqlx::query("INSERT INTO metadata_values(id,media_item_id,field_key,locale,value_json,provider_id,source_url,fetched_at,is_user_modified,is_locked) VALUES (?,?,?,?,?,?,?,?,0,0) ON CONFLICT(id) DO UPDATE SET value_json=excluded.value_json,provider_id=excluded.provider_id,source_url=excluded.source_url,fetched_at=excluded.fetched_at WHERE metadata_values.is_locked=0 AND metadata_values.is_user_modified=0").bind(value_id).bind(media.id.to_string()).bind(&diff.key).bind(locale).bind(json(&diff.new.value)?).bind(&diff.provider_id).bind(&diff.source_url).bind(Utc::now().timestamp_millis()).execute(&mut *tx).await?;
            match diff.key.as_str() {
                "title" => title = diff.new.value.as_str().map(str::to_owned),
                "original_title" => original = diff.new.value.as_str().map(str::to_owned),
                "year" => year = diff.new.value.as_i64().and_then(|v| i32::try_from(v).ok()),
                _ => {}
            }
            applied += 1;
        }
        let state: String = sqlx::query_scalar("SELECT state FROM scrape_tasks WHERE id=?")
            .bind(run_id.to_string())
            .fetch_one(&mut *tx)
            .await?;
        if state != "waiting_for_user" || cancel.is_cancelled() {
            return Err(RepositoryError::Conflict);
        }
        // A translation can be added without overwriting an independently protected
        // local/neutral primary title or year.
        for key in ["title", "original_title", "year"] {
            let protected:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM metadata_values WHERE media_item_id=? AND field_key=? AND (is_locked=1 OR is_user_modified=1))").bind(media.id.to_string()).bind(key).fetch_one(&mut *tx).await?;
            if protected {
                match key {
                    "title" => title = None,
                    "original_title" => original = None,
                    "year" => year = None,
                    _ => {}
                }
            }
        }
        // Canonical display title is changed only by this explicitly confirmed merge.
        sqlx::query("UPDATE media_items SET title=?,original_title=?,year=?,updated_at=MAX(updated_at,?) WHERE id=?").bind(title.as_ref().unwrap_or(&media.title)).bind(original.as_ref().or(media.original_title.as_ref())).bind(year.or(media.year)).bind(Utc::now().timestamp_millis()).bind(media.id.to_string()).execute(&mut *tx).await?;
        for external in &detail.identity().external_ids {
            let existing:Option<String>=sqlx::query_scalar("SELECT external_id FROM external_ids WHERE media_item_id=? AND namespace=? LIMIT 1").bind(media.id.to_string()).bind(&external.namespace).fetch_optional(&mut *tx).await?;
            if existing.as_ref().is_some_and(|v| v != &external.value) {
                return Err(RepositoryError::Conflict);
            }
            sqlx::query("INSERT OR IGNORE INTO external_ids(id,media_item_id,provider_id,namespace,external_id,is_primary,created_at) VALUES (?,?,?,?,?,0,?)").bind(Uuid::new_v4().to_string()).bind(media.id.to_string()).bind(&detail.identity().provider_id).bind(&external.namespace).bind(&external.value).bind(Utc::now().timestamp_millis()).execute(&mut *tx).await?;
        }
        for (art, cached) in artworks {
            sqlx::query("INSERT INTO artworks(id,media_item_id,kind,cache_path,source_url,provider_id,width,height,content_hash,created_at) VALUES (?,?,?,?,?,?,?,?,?,?)").bind(Uuid::new_v4().to_string()).bind(media.id.to_string()).bind(art.kind).bind(cached.cache_path).bind(art.url).bind(art.source_provider).bind(i64::from(cached.width)).bind(i64::from(cached.height)).bind(cached.content_hash).bind(Utc::now().timestamp_millis()).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO video_metadata_sources(media_item_id,media_kind,metadata_json,merge_preview_id,updated_at) VALUES (?,?,?,?,?) ON CONFLICT(media_item_id) DO UPDATE SET media_kind=excluded.media_kind,metadata_json=excluded.metadata_json,merge_preview_id=excluded.merge_preview_id,updated_at=excluded.updated_at").bind(media.id.to_string()).bind(media.kind.as_str()).bind(json(&detail)?).bind(id.to_string()).bind(Utc::now().timestamp_millis()).execute(&mut *tx).await?;
        // Resolving an automatic refresh exception manually clears its live
        // attention state; task/search history still retains the original run.
        sqlx::query("UPDATE library_refresh_items SET state='matched',reason='identified',run_id=?,updated_at=? WHERE media_item_id=? AND state IN ('pending','needs_review','failed')")
            .bind(run_id.to_string()).bind(Utc::now().timestamp_millis()).bind(media.id.to_string()).execute(&mut *tx).await?;
        sqlx::query("UPDATE scan_sessions SET matched_count=(SELECT count(*) FROM library_refresh_items i WHERE i.task_id=scan_sessions.task_id AND i.state='matched'),review_count=(SELECT count(*) FROM library_refresh_items i WHERE i.task_id=scan_sessions.task_id AND i.state='needs_review'),metadata_failed_count=(SELECT count(*) FROM library_refresh_items i WHERE i.task_id=scan_sessions.task_id AND i.state='failed') WHERE mode='automatic' AND task_id IN (SELECT task_id FROM library_refresh_items WHERE media_item_id=?)")
            .bind(media.id.to_string()).execute(&mut *tx).await?;
        sqlx::query("UPDATE scrape_tasks SET state='succeeded',updated_at=MAX(updated_at,?) WHERE state='waiting_for_user' AND id IN (SELECT s.task_id FROM scan_sessions s JOIN library_refresh_items i ON i.task_id=s.task_id WHERE i.media_item_id=? AND s.mode='automatic' AND s.review_count=0 AND s.metadata_failed_count=0 AND NOT EXISTS(SELECT 1 FROM scan_candidates c WHERE c.task_id=s.task_id AND c.status='pending'))")
            .bind(Utc::now().timestamp_millis()).bind(media.id.to_string()).execute(&mut *tx).await?;
        let result = MergeResult {
            schema_version: 1,
            preview_id: id,
            applied_fields: applied,
            already_applied: false,
        };
        sqlx::query("UPDATE video_merge_previews SET state='succeeded',result_json=? WHERE id=?")
            .bind(json(&result)?)
            .bind(id.to_string())
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE scrape_tasks SET state='succeeded',updated_at=MAX(updated_at,?) WHERE id=? AND state='waiting_for_user'").bind(Utc::now().timestamp_millis()).bind(run_id.to_string()).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(result)
    }
    async fn ordinary_media(
        &self,
        id: Uuid,
        kind: MediaKind,
    ) -> Result<MediaItem, RepositoryError> {
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
        if media.kind != kind || isolated {
            return Err(RepositoryError::InvalidInput);
        }
        Ok(media)
    }
    async fn persist_records(&self, id: Uuid) -> Result<(), RepositoryError> {
        for record in self.engine.take_records(id).await {
            sqlx::query("INSERT INTO provider_requests(task_id,record_json) VALUES (?,?)")
                .bind(id.to_string())
                .bind(json(&record)?)
                .execute(self.repo.pool())
                .await?;
        }
        Ok(())
    }
    pub async fn search_preview(&self, id: Uuid) -> Result<SearchPreview, RepositoryError> {
        let (_, _, preview) = self.session(id).await?;
        Ok(preview)
    }
    pub async fn media_detail(&self, id: Uuid) -> Result<Option<VideoMetadata>, RepositoryError> {
        let media = self
            .repo
            .get_media(id)
            .await?
            .ok_or(RepositoryError::NotFound)?;
        if !matches!(
            media.kind,
            MediaKind::Movie | MediaKind::TvSeries | MediaKind::Anime
        ) {
            return Err(RepositoryError::InvalidInput);
        }
        self.ordinary_media(id, media.kind).await?;
        sqlx::query_scalar::<_, String>(
            "SELECT metadata_json FROM video_metadata_sources WHERE media_item_id=?",
        )
        .bind(id.to_string())
        .fetch_optional(self.repo.pool())
        .await?
        .map(|v| decode(&v))
        .transpose()
    }
    pub async fn history(&self) -> Result<Vec<ScraperHistoryEntry>, RepositoryError> {
        let rows=sqlx::query("SELECT v.*,t.state FROM video_scrape_sessions v JOIN scrape_tasks t ON t.id=v.task_id ORDER BY v.created_at DESC LIMIT 100").fetch_all(self.repo.pool()).await?;
        rows.into_iter()
            .map(|r| {
                Ok(ScraperHistoryEntry {
                    id: Uuid::parse_str(&r.try_get::<String, _>("task_id")?)
                        .map_err(|_| RepositoryError::CorruptData)?,
                    query: decode(&r.try_get::<String, _>("query_json")?)?,
                    state: r.try_get("state")?,
                    created_at: chrono::DateTime::from_timestamp_millis(r.try_get("created_at")?)
                        .ok_or(RepositoryError::CorruptData)?,
                    candidate_count: r
                        .try_get::<Option<String>, _>("search_json")?
                        .map(|s| decode::<SearchPreview>(&s).map(|p| p.candidates.len()))
                        .transpose()?
                        .unwrap_or(0),
                })
            })
            .collect()
    }
}
fn json<T: Serialize>(value: &T) -> Result<String, RepositoryError> {
    serde_json::to_string(value).map_err(|_| RepositoryError::InvalidInput)
}
fn decode<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, RepositoryError> {
    serde_json::from_str(value).map_err(|_| RepositoryError::CorruptData)
}
fn query_kind(q: &VideoQuery) -> MediaKind {
    match q {
        VideoQuery::Movie { .. } => MediaKind::Movie,
        VideoQuery::Tv { .. } => MediaKind::TvSeries,
        VideoQuery::Anime { .. } => MediaKind::Anime,
    }
}
fn metadata_kind(m: &VideoMetadata) -> MediaKind {
    match m {
        VideoMetadata::Movie(_) => MediaKind::Movie,
        VideoMetadata::Tv(_) => MediaKind::TvSeries,
        VideoMetadata::Anime(_) => MediaKind::Anime,
    }
}
fn empty_value(v: &serde_json::Value) -> bool {
    v.is_null()
        || v.as_str().is_some_and(|s| s.trim().is_empty())
        || v.as_array().is_some_and(Vec::is_empty)
}
fn validate_query(q: &VideoQuery) -> Result<(), RepositoryError> {
    let (title, year, ids, language) = q.parts();
    if !title.trim().is_empty() {
        crate::providers::validate_query(title)?;
    }
    if title.len() > 256
        || title.chars().any(char::is_control)
        || title.contains("://")
        || (title.trim().is_empty() && ids.is_empty())
        || year.is_some_and(|y| !(1800..=2200).contains(&y))
        || ids.len() > 8
        || ids.iter().any(|id| {
            id.namespace.len() > 32
                || id.value.is_empty()
                || id.value.len() > 128
                || !id
                    .value
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
        })
        || normalize_locale(Some(language)).is_err()
    {
        return Err(RepositoryError::InvalidInput);
    }
    Ok(())
}
fn validate_fields(fields: &[RemoteField]) -> Result<(), RepositoryError> {
    let mut seen = HashSet::new();
    for f in fields {
        if ![
            "title",
            "original_title",
            "year",
            "overview",
            "runtime_minutes",
            "genres",
            "rating",
            "status",
            "format",
            "season",
            "episodes",
            "aliases",
            "studios",
            "seasons",
            "production_companies",
            "cast",
            "directors",
            "source",
            "duration",
            "license",
            "attribution",
            "source_license",
        ]
        .contains(&f.key.as_str())
            || !seen.insert((
                f.key.clone(),
                normalize_locale(f.language.as_deref())
                    .map_err(|_| RepositoryError::InvalidInput)?,
            ))
            || json(&f.value)?.len() > 256_000
        {
            return Err(RepositoryError::InvalidInput);
        }
    }
    Ok(())
}
fn normalized(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}
/// Separate alias matching for Anime; ordinary-video matching never consumes aliases.
pub fn match_candidate(
    query: &VideoQuery,
    candidate: VideoCandidate,
    previous: &[MatchedCandidate],
) -> MatchedCandidate {
    let (title, year, ids, _) = query.parts();
    let data = candidate.data();
    let same_id = ids.iter().any(|a| data.external_ids.iter().any(|b| a == b));
    let id_conflict = ids.iter().any(|a| {
        data.external_ids
            .iter()
            .any(|b| a.namespace == b.namespace && a.value != b.value)
    });
    let year_conflict = year.zip(data.year).is_some_and(|(a, b)| a != b);
    let mut reasons = vec![];
    if same_id {
        reasons.push("external_id_exact".into())
    }
    if id_conflict {
        reasons.push("external_id_conflict".into())
    }
    if year_conflict {
        reasons.push("year_conflict".into())
    }
    let title_match = normalized(title) == normalized(&data.title)
        || data
            .original_title
            .as_ref()
            .is_some_and(|s| normalized(s) == normalized(title));
    let alias_match = matches!(query, VideoQuery::Anime { .. })
        && data
            .aliases
            .iter()
            .any(|s| normalized(s) == normalized(title));
    if title_match {
        reasons.push("title_exact".into())
    }
    if alias_match {
        reasons.push("anime_alias_exact".into())
    }
    if !same_id {
        reasons.push("manual_selection_required".into())
    }
    let duplicate_of = previous.iter().position(|p| {
        let b = p.candidate.data();
        p.candidate.kind() == candidate.kind()
            && !data.external_ids.iter().any(|a| {
                b.external_ids
                    .iter()
                    .any(|v| a.namespace == v.namespace && a.value != v.value)
            })
            && data.external_ids.iter().any(|a| b.external_ids.contains(a))
            && data.year.zip(b.year).is_none_or(|(a, b)| a == b)
    });
    MatchedCandidate {
        candidate,
        reasons,
        needs_review: !same_id || id_conflict || year_conflict,
        duplicate_of,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        credentials::{CredentialMode, CredentialStore},
        library::MetadataWriteMode,
        network::SecretString,
        providers::TypedVideoRegistration,
    };
    use std::{
        sync::atomic::{AtomicBool, Ordering},
        time::Duration,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    struct Setup {
        dir: tempfile::TempDir,
        repo: LibraryRepository,
        service: ScraperService,
        providers: ProviderService,
        media: MediaItem,
        server: tokio::task::JoinHandle<()>,
        slow: Arc<AtomicBool>,
        base: String,
    }
    async fn setup() -> Setup {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let slow = Arc::new(AtomicBool::new(false));
        let delayed = slow.clone();
        let server = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let delayed = delayed.clone();
                tokio::spawn(async move {
                    let mut request = vec![0; 8192];
                    let count = stream.read(&mut request).await.unwrap();
                    let request = String::from_utf8_lossy(&request[..count]);
                    let path = request.lines().next().unwrap_or("").to_owned();
                    let body = if path.contains("/search/tv") {
                        r#"{"results":[{"id":52,"name":"Example Show","original_name":"Example Show","first_air_date":"2020-01-01","adult":false}]}"#
                    } else if path.contains("/tv/52") {
                        r#"{"id":52,"name":"Confirmed show","original_name":"Example Show","first_air_date":"2020-01-01","adult":false,"overview":"Provider TV overview","genres":[{"name":"Drama"}],"production_companies":[],"credits":{"cast":[]},"images":{"posters":[],"backdrops":[]},"external_ids":{"tvdb_id":52},"seasons":[]}"#
                    } else if path.contains("/search/movie") {
                        r#"{"results":[{"id":42,"title":"Example","original_title":"Example","release_date":"2020-01-01","adult":false},{"id":43,"title":"Example","release_date":"1980-01-01","adult":false}]}"#
                    } else if path.contains("/movie/42") {
                        if delayed.load(Ordering::SeqCst) {
                            tokio::time::sleep(Duration::from_millis(500)).await;
                        }
                        r#"{"id":42,"title":"Provider title","original_title":"Original","release_date":"2020-01-01","adult":false,"overview":"Provider overview","runtime":120,"genres":[{"name":"Drama"}],"production_companies":[],"credits":{"cast":[]},"images":{"posters":[],"backdrops":[]},"external_ids":{"imdb_id":"tt0042"}}"#
                    } else {
                        r#"{"errors":[{"message":"mock provider unavailable"}]}"#
                    };
                    let response=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len());
                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        });
        let dir = crate::test_support::tempdir().unwrap();
        let repo = LibraryRepository::open(&dir.path().join("library.sqlite"))
            .await
            .unwrap();
        let engine = NetworkEngine::for_loopback(4);
        let providers = ProviderService::with_engine(repo.clone(), engine)
            .await
            .unwrap();
        for d in VideoProviderHub::descriptors() {
            providers
                .register_adapter(Arc::new(TypedVideoRegistration(d)))
                .await
                .unwrap();
        }
        for id in ["tmdb", "anilist"] {
            providers
                .update_config(
                    id,
                    &ProviderConfig {
                        enabled: true,
                        interval_ms: 0,
                        max_retries: 0,
                        timeout_ms: 2000,
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
        }
        let credentials = Arc::new(CredentialStore::new());
        credentials
            .put(
                "tmdb.access_token",
                SecretString::new("synthetic-mock-only".into()),
                CredentialMode::SessionOnly,
            )
            .await
            .unwrap();
        let base = format!("http://{addr}");
        let hub = VideoProviderHub::new(providers.engine(), credentials)
            .with_credential_mode(CredentialMode::SessionOnly)
            .with_mock_endpoints(&base, &base, &base);
        let now = Utc::now();
        let media = MediaItem {
            id: Uuid::new_v4(),
            kind: MediaKind::Movie,
            title: "Local title".into(),
            original_title: None,
            year: Some(2020),
            created_at: now,
            updated_at: now,
        };
        repo.save_media(&media).await.unwrap();
        let root = LibraryRoot {
            id: Uuid::new_v4(),
            name: "Mock media".into(),
            path: dir.path().to_string_lossy().into_owned(),
            path_style: crate::test_support::native_path_style(),
            kind: Some(MediaKind::Movie),
            enabled: true,
            created_at: now,
            updated_at: now,
        };
        repo.save_root(&root).await.unwrap();
        std::fs::write(
            dir.path().join("Example.mkv"),
            b"synthetic source media must stay intact",
        )
        .unwrap();
        repo.save_file(&MediaFile {
            id: Uuid::new_v4(),
            media_item_id: media.id,
            root_id: root.id,
            relative_path: "Example.mkv".into(),
            size_bytes: 38,
            modified_at: None,
            part: None,
        })
        .await
        .unwrap();
        let service = ScraperService::new(
            repo.clone(),
            providers.clone(),
            Arc::new(hub),
            providers.engine(),
            dir.path().join("cache"),
        );
        service.initialize().await.unwrap();
        Setup {
            dir,
            repo,
            service,
            providers,
            media,
            server,
            slow,
            base,
        }
    }
    fn request() -> ScraperSearchRequest {
        ScraperSearchRequest {
            query: VideoQuery::Movie {
                title: "Example".into(),
                year: Some(2020),
                external_ids: vec![],
                language: "en-US".into(),
            },
            providers: vec!["tmdb".into()],
        }
    }
    fn selection(run_id: Uuid, media_item_id: Uuid) -> MergeRequest {
        MergeRequest {
            run_id,
            candidate_index: 0,
            media_item_id,
            selected_fields: vec!["title".into(), "overview".into(), "year".into()],
            selected_artwork: vec![],
        }
    }
    fn local_value(
        id: Uuid,
        key: &str,
        locale: Option<&str>,
        value: serde_json::Value,
    ) -> MetadataValue {
        MetadataValue {
            id: Uuid::new_v4(),
            media_item_id: id,
            field_key: key.into(),
            locale: locale.map(str::to_owned),
            value,
            provider_id: None,
            source_url: None,
            fetched_at: None,
            confidence: None,
            is_user_modified: true,
            is_locked: true,
        }
    }
    async fn settle_automatic(
        scans: &crate::scan_service::ScanService,
        id: Uuid,
    ) -> mediaatlas_application::library::ScanTaskView {
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let task = scans
                    .list_tasks(false)
                    .await
                    .unwrap()
                    .into_iter()
                    .find(|t| t.id == id)
                    .unwrap();
                if !matches!(task.state, TaskState::Queued | TaskState::Running) {
                    return task;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap()
    }
    #[tokio::test]
    async fn automatic_library_refresh_searches_and_merges_unique_match_without_sidecar_writes() {
        let s = setup().await;
        let protected = local_value(
            s.media.id,
            "overview",
            Some("zh-CN"),
            serde_json::json!("My protected overview"),
        );
        s.repo
            .save_metadata_value(&protected, MetadataWriteMode::User)
            .await
            .unwrap();
        let root = s.repo.list_roots().await.unwrap().remove(0);
        let original = std::fs::read(s.dir.path().join("Example.mkv")).unwrap();
        let scans =
            crate::scan_service::ScanService::new(s.repo.clone()).with_scraper(s.service.clone());
        let task = scans.start_automatic(root.id).await.unwrap();
        let finished = settle_automatic(&scans, task.id).await;
        assert_eq!(finished.state, TaskState::Succeeded);
        assert_eq!(finished.matched_count, 1);
        assert_eq!(
            s.repo.get_media(s.media.id).await.unwrap().unwrap().title,
            "Provider title"
        );
        assert!(s.service.media_detail(s.media.id).await.unwrap().is_some());
        assert_eq!(
            s.repo
                .list_metadata_values(s.media.id)
                .await
                .unwrap()
                .iter()
                .find(|v| v.field_key == "overview")
                .unwrap()
                .value,
            serde_json::json!("My protected overview")
        );
        assert_eq!(
            std::fs::read(s.dir.path().join("Example.mkv")).unwrap(),
            original
        );
        assert!(!s.dir.path().join("Example.nfo").exists());
        let searches = s.service.history().await.unwrap().len();
        let second = scans.start_automatic(root.id).await.unwrap();
        assert_eq!(
            settle_automatic(&scans, second.id).await.state,
            TaskState::Succeeded
        );
        assert_eq!(
            s.service.history().await.unwrap().len(),
            searches,
            "existing metadata is not re-fetched or overwritten on every refresh"
        );
        s.server.abort();
        s.repo.pool().close().await;
    }

    #[tokio::test]
    async fn automatic_refresh_unknown_year_is_visible_but_waits_for_identification() {
        let s = setup().await;
        sqlx::query("UPDATE media_items SET year=NULL WHERE id=?")
            .bind(s.media.id.to_string())
            .execute(s.repo.pool())
            .await
            .unwrap();
        let root = s.repo.list_roots().await.unwrap().remove(0);
        let scans =
            crate::scan_service::ScanService::new(s.repo.clone()).with_scraper(s.service.clone());
        let task = scans.start_automatic(root.id).await.unwrap();
        let finished = settle_automatic(&scans, task.id).await;
        assert_eq!(finished.state, TaskState::WaitingForUser);
        assert_eq!(finished.review_count, 1);
        assert_eq!(finished.matched_count, 0);
        assert!(s.service.media_detail(s.media.id).await.unwrap().is_none());
        let status: String =
            sqlx::query_scalar("SELECT reason FROM library_refresh_items WHERE task_id=?")
                .bind(task.id.to_string())
                .fetch_one(s.repo.pool())
                .await
                .unwrap();
        assert_eq!(status, "ambiguous_match");
        let run = s.service.history().await.unwrap().remove(0);
        let preview = s
            .service
            .preview(selection(run.id, s.media.id))
            .await
            .unwrap();
        s.service.confirm(preview.preview_id).await.unwrap();
        let resolved = scans
            .list_tasks(false)
            .await
            .unwrap()
            .into_iter()
            .find(|t| t.id == task.id)
            .unwrap();
        assert_eq!(resolved.review_count, 0);
        assert_eq!(resolved.matched_count, 1);
        assert_eq!(resolved.state, TaskState::Succeeded);
        s.server.abort();
        s.repo.pool().close().await;
    }

    #[tokio::test]
    async fn automatic_refresh_without_enabled_source_remains_unconfigured_until_real_retry() {
        let s = setup().await;
        let mut config = s.providers.config("tmdb").await.unwrap();
        config.enabled = false;
        s.providers.update_config("tmdb", &config).await.unwrap();
        let root = s.repo.list_roots().await.unwrap().remove(0);
        let scans =
            crate::scan_service::ScanService::new(s.repo.clone()).with_scraper(s.service.clone());
        let first = scans.start_automatic(root.id).await.unwrap();
        let pending = settle_automatic(&scans, first.id).await;
        assert_eq!(pending.state, TaskState::WaitingForUser);
        assert_eq!(pending.imported_count, 1);
        assert_eq!(pending.matched_count, 0);
        assert_eq!(pending.unconfigured_count, 1);
        assert_eq!(pending.review_count, 1);
        assert!(
            s.service.history().await.unwrap().is_empty(),
            "disabled source performs no remote scraping"
        );
        assert!(s.service.media_detail(s.media.id).await.unwrap().is_none());
        config = s.providers.config("tmdb").await.unwrap();
        config.enabled = true;
        s.providers.update_config("tmdb", &config).await.unwrap();
        assert_eq!(
            scans.list_tasks(false).await.unwrap()[0].unconfigured_count,
            1,
            "saving source config does not pretend scraping has occurred"
        );
        let retry = scans.start_automatic(root.id).await.unwrap();
        let completed = settle_automatic(&scans, retry.id).await;
        assert_eq!(completed.state, TaskState::Succeeded);
        assert_eq!(completed.unconfigured_count, 0);
        assert_eq!(completed.review_count, 0);
        assert_eq!(completed.matched_count, 1);
        assert!(s.service.media_detail(s.media.id).await.unwrap().is_some());
        assert_eq!(s.repo.list_files(s.media.id).await.unwrap().len(), 1);
        assert!(!s.dir.path().join("Example.nfo").exists());
        s.server.abort();
        s.repo.pool().close().await;
    }

    async fn wait_for_automatic_detail(repo: &LibraryRepository) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let details: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM video_scrape_sessions WHERE search_json IS NOT NULL",
                )
                .fetch_one(repo.pool())
                .await
                .unwrap();
                if details > 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn automatic_refresh_identifies_a_yearless_series_once_for_multiple_episodes() {
        let s = setup().await;
        let path = s.dir.path().join("shows");
        std::fs::create_dir(&path).unwrap();
        for episode in ["Example.Show.S01E01.mkv", "Example.Show.S01E02.mkv"] {
            std::fs::write(path.join(episode), b"read-only episode fixture").unwrap();
        }
        let root = crate::browse::BrowseService::new(s.repo.clone(), s.dir.path().join("cache"))
            .save_root(&mediaatlas_application::library::RootInput {
                id: None,
                name: "Shows".into(),
                path: path.to_string_lossy().into_owned(),
                kind: Some(MediaKind::TvSeries),
                enabled: true,
                exclude_patterns: vec![],
            })
            .await
            .unwrap();
        let scans =
            crate::scan_service::ScanService::new(s.repo.clone()).with_scraper(s.service.clone());
        let task = scans.start_automatic(root.root.id).await.unwrap();
        let finished = settle_automatic(&scans, task.id).await;
        assert_eq!(finished.state, TaskState::Succeeded);
        assert_eq!(finished.imported_count, 2);
        assert_eq!(finished.matched_count, 1);
        assert_eq!(s.service.history().await.unwrap().len(), 1);
        let count: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM video_metadata_sources WHERE media_kind='tv_series'",
        )
        .fetch_one(s.repo.pool())
        .await
        .unwrap();
        assert_eq!(count, 1);
        s.server.abort();
        s.repo.pool().close().await;
    }

    #[tokio::test]
    async fn automatic_refresh_cancellation_stops_network_and_does_not_publish_late_metadata() {
        let s = setup().await;
        s.slow.store(true, Ordering::SeqCst);
        let root = s.repo.list_roots().await.unwrap().remove(0);
        let scans =
            crate::scan_service::ScanService::new(s.repo.clone()).with_scraper(s.service.clone());
        let task = scans.start_automatic(root.id).await.unwrap();
        wait_for_automatic_detail(&s.repo).await;
        assert_eq!(
            scans.cancel(task.id).await.unwrap().state,
            TaskState::Cancelled
        );
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let pending: i64 = sqlx::query_scalar("SELECT count(*) FROM library_refresh_items WHERE task_id=? AND state='pending'")
                    .bind(task.id.to_string()).fetch_one(s.repo.pool()).await.unwrap();
                if pending == 0 { break; }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }).await.unwrap();
        assert!(s.service.media_detail(s.media.id).await.unwrap().is_none());
        assert_eq!(
            s.repo.get_media(s.media.id).await.unwrap().unwrap().title,
            "Example"
        );
        s.server.abort();
        s.repo.pool().close().await;
    }

    #[tokio::test]
    async fn automatic_refresh_rechecks_privacy_after_network_response() {
        let s = setup().await;
        s.slow.store(true, Ordering::SeqCst);
        let root = s.repo.list_roots().await.unwrap().remove(0);
        let scans =
            crate::scan_service::ScanService::new(s.repo.clone()).with_scraper(s.service.clone());
        let task = scans.start_automatic(root.id).await.unwrap();
        wait_for_automatic_detail(&s.repo).await;
        sqlx::query("UPDATE media_items SET adult_isolated=1 WHERE id=?")
            .bind(s.media.id.to_string())
            .execute(s.repo.pool())
            .await
            .unwrap();
        let finished = settle_automatic(&scans, task.id).await;
        assert_eq!(finished.metadata_failed_count, 1);
        let written: i64 =
            sqlx::query_scalar("SELECT count(*) FROM video_metadata_sources WHERE media_item_id=?")
                .bind(s.media.id.to_string())
                .fetch_one(s.repo.pool())
                .await
                .unwrap();
        assert_eq!(written, 0);
        s.server.abort();
        s.repo.pool().close().await;
    }

    #[tokio::test]
    async fn automatic_refresh_never_searches_a_direct_target_with_an_isolated_ancestor() {
        let s = setup().await;
        let now = Utc::now();
        let ancestor = MediaItem {
            id: Uuid::new_v4(),
            kind: MediaKind::AdultVideo,
            title: "Isolated parent".into(),
            original_title: None,
            year: None,
            created_at: now,
            updated_at: now,
        };
        s.repo.save_media(&ancestor).await.unwrap();
        sqlx::query("INSERT INTO media_relations(parent_id,child_id,relation_type) VALUES(?,?,'anime_base')")
            .bind(ancestor.id.to_string()).bind(s.media.id.to_string()).execute(s.repo.pool()).await.unwrap();
        let root = s.repo.list_roots().await.unwrap().remove(0);
        let scans =
            crate::scan_service::ScanService::new(s.repo.clone()).with_scraper(s.service.clone());
        let task = scans.start_automatic(root.id).await.unwrap();
        settle_automatic(&scans, task.id).await;
        assert!(
            s.service.history().await.unwrap().is_empty(),
            "isolated ancestors must be checked before network search"
        );
        s.server.abort();
        s.repo.pool().close().await;
    }

    #[tokio::test]
    async fn manual_identification_wins_over_a_late_automatic_result() {
        let s = setup().await;
        let manual = s.service.search(request()).await.unwrap();
        s.service.detail(manual.run_id, 0).await.unwrap();
        s.slow.store(true, Ordering::SeqCst);
        let root = s.repo.list_roots().await.unwrap().remove(0);
        let scans =
            crate::scan_service::ScanService::new(s.repo.clone()).with_scraper(s.service.clone());
        let task = scans.start_automatic(root.id).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let searched: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM video_scrape_sessions WHERE search_json IS NOT NULL",
                )
                .fetch_one(s.repo.pool())
                .await
                .unwrap();
                if searched > 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let manual_preview = s
            .service
            .preview(selection(manual.run_id, s.media.id))
            .await
            .unwrap();
        s.service.confirm(manual_preview.preview_id).await.unwrap();
        let finished = settle_automatic(&scans, task.id).await;
        assert_eq!(finished.state, TaskState::Succeeded);
        assert_eq!(finished.metadata_failed_count, 0);
        assert_eq!(finished.review_count, 0);
        let owner: String =
            sqlx::query_scalar("SELECT run_id FROM library_refresh_items WHERE task_id=?")
                .bind(task.id.to_string())
                .fetch_one(s.repo.pool())
                .await
                .unwrap();
        assert_eq!(owner, manual.run_id.to_string());
        s.server.abort();
        s.repo.pool().close().await;
    }
    #[tokio::test]
    async fn real_mock_tmdb_search_detail_merge_and_preserve_output_are_persistent_and_idempotent()
    {
        let s = setup().await;
        let mut query = request();
        query.providers.push("anilist".into());
        let search = s.service.search(query).await.unwrap();
        assert_eq!(search.candidates.len(), 2);
        assert!(search.failures.iter().any(|f| f.provider_id == "anilist"));
        assert!(search.candidates[1]
            .reasons
            .iter()
            .any(|r| r == "year_conflict"));
        let detail = s.service.detail(search.run_id, 0).await.unwrap();
        assert!(matches!(detail, VideoMetadata::Movie(_)));
        assert!(detail
            .fields()
            .iter()
            .any(|f| f.key == "production_companies"));
        let preview = s
            .service
            .preview(selection(search.run_id, s.media.id))
            .await
            .unwrap();
        assert!(preview
            .fields
            .iter()
            .any(|d| d.key == "title" && d.selected));
        assert_eq!(
            s.repo.get_media(s.media.id).await.unwrap().unwrap().title,
            "Local title"
        );
        let result = s.service.confirm(preview.preview_id).await.unwrap();
        assert_eq!(result.applied_fields, 3);
        assert!(s.service.media_detail(s.media.id).await.unwrap().is_some());
        assert_eq!(
            s.service
                .search_preview(search.run_id)
                .await
                .unwrap()
                .candidates
                .len(),
            2
        );
        assert_eq!(
            s.repo.get_media(s.media.id).await.unwrap().unwrap().title,
            "Provider title"
        );
        assert!(
            s.service
                .confirm(preview.preview_id)
                .await
                .unwrap()
                .already_applied
        );
        assert_eq!(s.repo.list_external_ids(s.media.id).await.unwrap().len(), 2);
        let output = crate::media_output::MediaOutputService::new(
            s.repo.clone(),
            s.dir.path().join("cache"),
        );
        let output_preview = output.preview(s.media.id).await.unwrap();
        assert!(!s.dir.path().join("Example.nfo").exists());
        output.execute(output_preview.preview_id).await.unwrap();
        assert!(std::fs::read_to_string(s.dir.path().join("Example.nfo"))
            .unwrap()
            .contains("Provider title"));
        assert!(
            output
                .execute(output_preview.preview_id)
                .await
                .unwrap()
                .already_applied
        );
        assert_eq!(
            std::fs::read(s.dir.path().join("Example.mkv")).unwrap(),
            b"synthetic source media must stay intact"
        );
        s.server.abort();
        let restored = ScraperService::new(
            s.repo.clone(),
            s.providers.clone(),
            s.service.hub.clone(),
            s.providers.engine(),
            s.dir.path().join("cache"),
        );
        assert_eq!(restored.history().await.unwrap()[0].candidate_count, 2);
        assert_eq!(restored.detail(search.run_id, 0).await.unwrap(), detail);
    }
    #[tokio::test]
    async fn neutral_local_nfo_and_other_locale_manual_titles_survive_translation_merge() {
        let s = setup().await;
        let protected = local_value(s.media.id, "title", None, serde_json::json!("Local title"));
        s.repo
            .save_metadata_value(&protected, MetadataWriteMode::User)
            .await
            .unwrap();
        let fr = local_value(
            s.media.id,
            "overview",
            Some("fr-FR"),
            serde_json::json!("Texte utilisateur"),
        );
        s.repo
            .save_metadata_value(&fr, MetadataWriteMode::User)
            .await
            .unwrap();
        let search = s.service.search(request()).await.unwrap();
        let p = s
            .service
            .preview(selection(search.run_id, s.media.id))
            .await
            .unwrap();
        s.service.confirm(p.preview_id).await.unwrap();
        assert_eq!(
            s.repo.get_media(s.media.id).await.unwrap().unwrap().title,
            "Local title"
        );
        let fields = s.repo.list_metadata_values(s.media.id).await.unwrap();
        assert!(fields.iter().any(|v| v.locale.as_deref() == Some("en-us")
            && v.field_key == "title"
            && v.value == "Provider title"));
        assert!(fields
            .iter()
            .any(|v| v.locale.as_deref() == Some("fr-fr") && v.value == "Texte utilisateur"));
        assert!(fields
            .iter()
            .any(|v| v.id == protected.id && v.is_protected()));
        s.server.abort();
    }
    #[tokio::test]
    async fn lock_after_preview_and_changed_before_image_prevent_atomic_merge() {
        let s = setup().await;
        let search = s.service.search(request()).await.unwrap();
        let p = s
            .service
            .preview(selection(search.run_id, s.media.id))
            .await
            .unwrap();
        let protected = local_value(
            s.media.id,
            "overview",
            Some("EN-us"),
            serde_json::json!("new user edit"),
        );
        s.repo
            .save_metadata_value(&protected, MetadataWriteMode::User)
            .await
            .unwrap();
        assert!(matches!(
            s.service.confirm(p.preview_id).await,
            Err(RepositoryError::ProtectedMetadata)
        ));
        assert_eq!(
            s.repo.get_media(s.media.id).await.unwrap().unwrap().title,
            "Local title"
        );
        assert!(!s
            .repo
            .list_metadata_values(s.media.id)
            .await
            .unwrap()
            .iter()
            .any(|v| v.field_key == "title"));
        s.server.abort();
    }
    #[tokio::test]
    async fn wrong_media_type_and_adult_isolation_are_rejected_at_preview_and_confirm() {
        let s = setup().await;
        let search = s.service.search(request()).await.unwrap();
        let mut other = s.media.clone();
        other.kind = MediaKind::Anime;
        s.repo.save_media(&other).await.unwrap();
        assert!(s
            .service
            .preview(selection(search.run_id, s.media.id))
            .await
            .is_err());
        s.repo.save_media(&s.media).await.unwrap();
        let p = s
            .service
            .preview(selection(search.run_id, s.media.id))
            .await
            .unwrap();
        sqlx::query("UPDATE media_items SET adult_isolated=1 WHERE id=?")
            .bind(s.media.id.to_string())
            .execute(s.repo.pool())
            .await
            .unwrap();
        assert!(s.service.confirm(p.preview_id).await.is_err());
        assert!(crate::media_output::MediaOutputService::new(
            s.repo.clone(),
            s.dir.path().join("cache")
        )
        .preview(s.media.id)
        .await
        .is_err());
        s.server.abort();
    }
    #[tokio::test]
    async fn cancel_inflight_detail_preserves_library_and_records_cancelled_state() {
        let s = setup().await;
        let search = s.service.search(request()).await.unwrap();
        s.slow.store(true, Ordering::SeqCst);
        let service = s.service.clone();
        let task = tokio::spawn(async move { service.detail(search.run_id, 0).await });
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(s.providers.cancel(search.run_id).await.unwrap());
        assert!(task.await.unwrap().is_err());
        assert_eq!(s.service.history().await.unwrap()[0].state, "cancelled");
        assert_eq!(
            s.repo.get_media(s.media.id).await.unwrap().unwrap().title,
            "Local title"
        );
        let records = s.providers.snapshot().await.unwrap().requests;
        assert!(records
            .iter()
            .any(|r| r.failure == Some(ProviderFailure::Cancelled)));
        s.server.abort();
    }
    #[test]
    fn type_specific_matching_never_deduplicates_conflicting_ids_or_years() {
        let q = request().query;
        let data = CandidateData {
            provider_id: "mock".into(),
            remote_id: "42".into(),
            title: "Example".into(),
            original_title: None,
            year: Some(2020),
            aliases: vec!["Alias".into()],
            external_ids: vec![RemoteId {
                namespace: "imdb".into(),
                value: "tt42".into(),
            }],
            source_url: "https://example.com".into(),
            language: "en-US".into(),
        };
        let a = match_candidate(&q, VideoCandidate::Movie(data.clone()), &[]);
        let mut conflict = data.clone();
        conflict.external_ids.push(RemoteId {
            namespace: "tmdb".into(),
            value: "1".into(),
        });
        let p = match_candidate(&q, VideoCandidate::Movie(conflict), &[]);
        let mut second = data.clone();
        second.external_ids.push(RemoteId {
            namespace: "tmdb".into(),
            value: "2".into(),
        });
        assert!(match_candidate(&q, VideoCandidate::Movie(second), &[p])
            .duplicate_of
            .is_none());
        let mut remake = data.clone();
        remake.year = Some(1980);
        assert!(match_candidate(&q, VideoCandidate::Movie(remake), &[a])
            .duplicate_of
            .is_none());
        let anime = VideoQuery::Anime {
            title: "Alias".into(),
            year: None,
            external_ids: vec![],
            language: "en-US".into(),
        };
        assert!(
            match_candidate(&anime, VideoCandidate::Anime(data.clone()), &[])
                .reasons
                .contains(&"anime_alias_exact".into())
        );
        assert!(!match_candidate(&q, VideoCandidate::Movie(data), &[])
            .reasons
            .contains(&"anime_alias_exact".into()));
    }
    #[test]
    fn internal_fields_and_empty_payloads_cannot_be_provider_merge_inputs() {
        assert!(validate_fields(&[RemoteField {
            key: "adult_isolated".into(),
            value: serde_json::json!(false),
            language: None
        }])
        .is_err());
        assert!(empty_value(&serde_json::json!(" ")));
        assert!(empty_value(&serde_json::Value::Null));
    }
    async fn gallery_fixture(
        s: &Setup,
    ) -> (Uuid, tokio::task::JoinHandle<()>, Arc<tokio::sync::Notify>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let started = Arc::new(tokio::sync::Notify::new());
        let signal = started.clone();
        let server = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let signal = signal.clone();
                tokio::spawn(async move {
                    let mut b = [0; 2048];
                    let _ = stream.read(&mut b).await;
                    signal.notify_one();
                    tokio::time::sleep(Duration::from_millis(400)).await;
                    let image = image::DynamicImage::new_rgb8(2, 2);
                    let mut bytes = std::io::Cursor::new(Vec::new());
                    image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
                    let body = bytes.into_inner();
                    let response=format!("HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",body.len());
                    let _ = stream.write_all(response.as_bytes()).await;
                    let _ = stream.write_all(&body).await;
                });
            }
        });
        let search = s.service.search(request()).await.unwrap();
        let identity = search.candidates[0].candidate.data().clone();
        // Explicit synthetic gallery fixture: production URLs remain supplied by real adapters.
        let detail = VideoMetadata::Movie(MovieDetail {
            identity,
            fields: vec![RemoteField {
                key: "title".into(),
                value: serde_json::json!("Gallery title"),
                language: Some("en-US".into()),
            }],
            artwork: vec![
                ArtworkCandidate {
                    url: format!("http://{addr}/one.png"),
                    source_provider: "tmdb".into(),
                    kind: "poster".into(),
                    language: None,
                },
                ArtworkCandidate {
                    url: format!("http://{addr}/two.png"),
                    source_provider: "tmdb".into(),
                    kind: "backdrop".into(),
                    language: None,
                },
            ],
            runtime_minutes: None,
        });
        sqlx::query(
            "INSERT INTO video_scrape_details(task_id,candidate_index,detail_json) VALUES (?,0,?)",
        )
        .bind(search.run_id.to_string())
        .bind(json(&detail).unwrap())
        .execute(s.repo.pool())
        .await
        .unwrap();
        (search.run_id, server, started)
    }

    #[tokio::test]
    async fn automatic_refresh_retries_failed_artwork_without_replacing_saved_metadata() {
        let s = setup().await;
        let (manual_run, image_server, _) = gallery_fixture(&s).await;
        let gallery = s.service.detail(manual_run, 0).await.unwrap();
        s.slow.store(true, Ordering::SeqCst);
        let root = s.repo.list_roots().await.unwrap().remove(0);
        let scans =
            crate::scan_service::ScanService::new(s.repo.clone()).with_scraper(s.service.clone());
        async fn inject_next_gallery(s: &Setup, previous: usize, gallery: &VideoMetadata) {
            let run = tokio::time::timeout(Duration::from_secs(5), async {
                loop {
                    let runs = s.service.history().await.unwrap();
                    if runs.len() > previous {
                        return runs[0].id;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap();
            // Synthetic provider detail fixture inserted before the delayed
            // adapter finishes; production still uses immutable adapter results.
            sqlx::query("INSERT OR IGNORE INTO video_scrape_details(task_id,candidate_index,detail_json) VALUES (?,0,?)")
                .bind(run.to_string()).bind(json(gallery).unwrap()).execute(s.repo.pool()).await.unwrap();
        }
        let mut broken = gallery.clone();
        if let VideoMetadata::Movie(detail) = &mut broken {
            for art in &mut detail.artwork {
                art.url = "http://127.0.0.1:1/unavailable.png".into();
            }
        }
        let first = scans.start_automatic(root.id).await.unwrap();
        inject_next_gallery(&s, 1, &broken).await;
        let failed = settle_automatic(&scans, first.id).await;
        assert_eq!(failed.metadata_failed_count, 1);
        assert!(
            s.service.media_detail(s.media.id).await.unwrap().is_some(),
            "a bad image does not discard matching metadata"
        );
        assert!(s.repo.list_artworks(s.media.id).await.unwrap().is_empty());
        let before = s.repo.get_media(s.media.id).await.unwrap().unwrap().title;
        let second = scans.start_automatic(root.id).await.unwrap();
        inject_next_gallery(&s, 2, &gallery).await;
        let repaired = settle_automatic(&scans, second.id).await;
        assert_eq!(repaired.state, TaskState::Succeeded);
        assert_eq!(repaired.metadata_failed_count, 0);
        assert_eq!(s.repo.list_artworks(s.media.id).await.unwrap().len(), 2);
        assert_eq!(
            s.repo.get_media(s.media.id).await.unwrap().unwrap().title,
            before
        );
        assert!(!s.dir.path().join("Example.nfo").exists());
        image_server.abort();
        s.server.abort();
        s.repo.pool().close().await;
    }
    #[tokio::test]
    async fn concurrent_gallery_requests_share_cancellation_and_never_apply_library_changes() {
        let s = setup().await;
        let (run, server, started) = gallery_fixture(&s).await;
        let first = s.service.clone();
        let a = tokio::spawn(async move { first.artwork_preview(run, 0, 0).await });
        started.notified().await;
        let second = s.service.clone();
        let b = tokio::spawn(async move { second.artwork_preview(run, 0, 1).await });
        tokio::time::sleep(Duration::from_millis(25)).await;
        assert!(s.providers.cancel(run).await.unwrap());
        assert!(a.await.unwrap().is_err());
        assert!(b.await.unwrap().is_err());
        assert!(s.repo.list_artworks(s.media.id).await.unwrap().is_empty());
        let records = s.providers.snapshot().await.unwrap().requests;
        assert_eq!(
            records
                .iter()
                .filter(|r| r.engine == "image" && r.failure == Some(ProviderFailure::Cancelled))
                .count(),
            2
        );
        server.abort();
        s.server.abort();
    }
    #[tokio::test]
    async fn cancelling_selected_artwork_during_confirm_rolls_back_all_metadata() {
        let s = setup().await;
        let (run, server, started) = gallery_fixture(&s).await;
        let p = s
            .service
            .preview(MergeRequest {
                run_id: run,
                candidate_index: 0,
                media_item_id: s.media.id,
                selected_fields: vec!["title".into()],
                selected_artwork: vec![0],
            })
            .await
            .unwrap();
        let service = s.service.clone();
        let task = tokio::spawn(async move { service.confirm(p.preview_id).await });
        started.notified().await;
        s.providers.cancel(run).await.unwrap();
        assert!(task.await.unwrap().is_err());
        assert!(s
            .repo
            .list_metadata_values(s.media.id)
            .await
            .unwrap()
            .is_empty());
        assert!(s.repo.list_artworks(s.media.id).await.unwrap().is_empty());
        assert_eq!(
            s.repo.get_media(s.media.id).await.unwrap().unwrap().title,
            "Local title"
        );
        server.abort();
        s.server.abort();
    }
    #[tokio::test]
    async fn disabling_actual_provider_cancels_typed_run_and_keeps_other_providers_enabled() {
        let s = setup().await;
        let search = s.service.search(request()).await.unwrap();
        s.slow.store(true, Ordering::SeqCst);
        let service = s.service.clone();
        let task = tokio::spawn(async move { service.detail(search.run_id, 0).await });
        tokio::time::sleep(Duration::from_millis(30)).await;
        s.providers.set_enabled("tmdb", false).await.unwrap();
        assert!(task.await.unwrap().is_err());
        assert_eq!(s.service.history().await.unwrap()[0].state, "cancelled");
        assert!(s.providers.config("anilist").await.unwrap().enabled);
        s.server.abort();
    }
    #[tokio::test]
    async fn selected_gallery_stores_correct_kinds_and_library_uses_only_local_thumbnail() {
        let s = setup().await;
        let (run, server, _) = gallery_fixture(&s).await;
        let p = s
            .service
            .preview(MergeRequest {
                run_id: run,
                candidate_index: 0,
                media_item_id: s.media.id,
                selected_fields: vec!["title".into()],
                selected_artwork: vec![0, 1],
            })
            .await
            .unwrap();
        s.service.confirm(p.preview_id).await.unwrap();
        let artwork = s.repo.list_artworks(s.media.id).await.unwrap();
        assert_eq!(artwork.len(), 2);
        assert!(artwork.iter().any(|a| a.kind == ArtworkKind::Poster));
        assert!(artwork.iter().any(|a| a.kind == ArtworkKind::Backdrop));
        assert_eq!(artwork[0].content_hash, artwork[1].content_hash);
        let browse =
            crate::browse::BrowseService::new(s.repo.clone(), s.dir.path().join("thumbnails"))
                .with_artwork_cache(s.dir.path().join("cache"));
        let detail = browse
            .get_media(&mediaatlas_application::library::MediaDetailInput {
                media_id: s.media.id,
                include_adult: false,
            })
            .await
            .unwrap();
        assert!(detail
            .poster_uri
            .unwrap()
            .starts_with("data:image/png;base64,"));
        let out = crate::media_output::MediaOutputService::new(
            s.repo.clone(),
            s.dir.path().join("cache"),
        );
        let plan = out.preview(s.media.id).await.unwrap();
        assert!(plan.files.iter().any(|f| f.kind == "poster"));
        out.execute(plan.preview_id).await.unwrap();
        assert!(s.dir.path().join("Example-poster.png").exists());
        assert_eq!(
            std::fs::read(s.dir.path().join("Example.mkv")).unwrap(),
            b"synthetic source media must stay intact"
        );
        server.abort();
        s.server.abort();
    }
    #[tokio::test]
    async fn canonical_edit_during_artwork_download_conflicts_even_with_same_timestamp() {
        let s = setup().await;
        let (run, server, started) = gallery_fixture(&s).await;
        let p = s
            .service
            .preview(MergeRequest {
                run_id: run,
                candidate_index: 0,
                media_item_id: s.media.id,
                selected_fields: vec!["title".into()],
                selected_artwork: vec![0],
            })
            .await
            .unwrap();
        let service = s.service.clone();
        let task = tokio::spawn(async move { service.confirm(p.preview_id).await });
        started.notified().await;
        sqlx::query("UPDATE media_items SET title='Concurrent local edit' WHERE id=?")
            .bind(s.media.id.to_string())
            .execute(s.repo.pool())
            .await
            .unwrap();
        assert!(matches!(
            task.await.unwrap(),
            Err(RepositoryError::Conflict)
        ));
        assert_eq!(
            s.repo.get_media(s.media.id).await.unwrap().unwrap().title,
            "Concurrent local edit"
        );
        assert!(s
            .repo
            .list_metadata_values(s.media.id)
            .await
            .unwrap()
            .is_empty());
        server.abort();
        s.server.abort();
    }
    #[tokio::test]
    async fn source_attribution_and_license_export_and_run_snapshots_are_immutable() {
        let s = setup().await;
        let search = s.service.search(request()).await.unwrap();
        assert!(
            sqlx::query("UPDATE video_scrape_sessions SET jobs_json='{}' WHERE task_id=?")
                .bind(search.run_id.to_string())
                .execute(s.repo.pool())
                .await
                .is_err()
        );
        assert!(
            sqlx::query("UPDATE video_scrape_sessions SET search_json='{}' WHERE task_id=?")
                .bind(search.run_id.to_string())
                .execute(s.repo.pool())
                .await
                .is_err()
        );
        for (key, text) in [
            (
                "attribution",
                "Wikipedia contributors; https://en.wikipedia.org/wiki/Example",
            ),
            (
                "source_license",
                "CC BY-SA 4.0; https://creativecommons.org/licenses/by-sa/4.0/",
            ),
        ] {
            let mut value = local_value(s.media.id, key, None, serde_json::json!(text));
            value.is_locked = false;
            value.is_user_modified = false;
            value.provider_id = Some("wikipedia.html".into());
            s.repo
                .save_metadata_value(&value, MetadataWriteMode::Provider)
                .await
                .unwrap();
        }
        let output = crate::media_output::MediaOutputService::new(
            s.repo.clone(),
            s.dir.path().join("cache"),
        );
        let p = output.preview(s.media.id).await.unwrap();
        let text = p.files[0].new_text.as_ref().unwrap();
        assert!(text.contains("<sourceattribution>Wikipedia contributors"));
        assert!(text.contains("<sourcelicense>CC BY-SA 4.0"));
        s.server.abort();
    }
    #[tokio::test]
    async fn settings_auth_revision_is_captured_by_scraper_jobs_and_never_serializes_credentials() {
        use crate::provider_auth::{AuthSaveInput, ProviderAuthService};
        let mut s = setup().await;
        let credentials = Arc::new(CredentialStore::new());
        let auth = ProviderAuthService::new(s.repo.clone(), credentials.clone());
        auth.save(AuthSaveInput {
            provider_id: "tmdb".into(),
            method: "api_key".into(),
            storage: "session_only".into(),
            secret: "synthetic-query-key".into(),
        })
        .await
        .unwrap();
        let hub = VideoProviderHub::new(s.providers.engine(), credentials)
            .with_auth_service(auth.clone())
            .with_mock_endpoints(&s.base, &s.base, &s.base);
        s.service = ScraperService::new(
            s.repo.clone(),
            s.providers.clone(),
            Arc::new(hub),
            s.providers.engine(),
            s.dir.path().join("cache"),
        )
        .with_auth_service(auth.clone());
        let old = s.service.search(request()).await.unwrap();
        let persisted: String =
            sqlx::query_scalar("SELECT jobs_json FROM video_scrape_sessions WHERE task_id=?")
                .bind(old.run_id.to_string())
                .fetch_one(s.repo.pool())
                .await
                .unwrap();
        assert!(persisted.contains("tmdb.auth.api_key.session."));
        assert!(!persisted.contains("synthetic-query-key"));
        auth.save(AuthSaveInput {
            provider_id: "tmdb".into(),
            method: "bearer".into(),
            storage: "session_only".into(),
            secret: "synthetic-bearer-value".into(),
        })
        .await
        .unwrap();
        assert!(matches!(
            s.service.detail(old.run_id, 0).await,
            Err(RepositoryError::Provider(
                ProviderFailure::CredentialsUnavailable
            ))
        ));
        let new = s.service.search(request()).await.unwrap();
        assert!(s.service.detail(new.run_id, 0).await.is_ok());
        let mut invalid = s.providers.config("tmdb").await.unwrap();
        invalid.credential_ref = Some("must-not-store-user-token".into());
        assert!(matches!(
            s.providers.update_config("tmdb", &invalid).await,
            Err(RepositoryError::InvalidInput)
        ));
        assert!(s
            .providers
            .config("tmdb")
            .await
            .unwrap()
            .credential_ref
            .is_none());
        s.server.abort();
    }
    #[tokio::test]
    async fn confirmed_tmdb_series_completes_only_typed_local_episode_hierarchy() {
        use mediaatlas_application::library::{
            BrowseQuery, ConfirmInput, MediaDetailInput, PreviewInput, RootInput,
        };
        let s = setup().await;
        let media_path = s.dir.path().join("tv");
        std::fs::create_dir(&media_path).unwrap();
        for name in [
            "Example.Show.S01E01.mkv",
            "Example.Show.S02E01.mkv",
            "Other.Show.S01E01.mkv",
        ] {
            std::fs::write(media_path.join(name), b"temporary episode fixture").unwrap();
        }
        let browse = crate::browse::BrowseService::new(s.repo.clone(), s.dir.path().join("thumbs"));
        let root = browse
            .save_root(&RootInput {
                id: None,
                name: "TV".into(),
                path: media_path.to_str().unwrap().into(),
                kind: Some(MediaKind::TvSeries),
                enabled: true,
                exclude_patterns: vec![],
            })
            .await
            .unwrap();
        let scans = crate::scan_service::ScanService::new(s.repo.clone());
        let scan = scans.start(root.root.id).await.unwrap();
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                if scans
                    .list_tasks(false)
                    .await
                    .unwrap()
                    .iter()
                    .any(|t| t.id == scan.id && t.state == TaskState::WaitingForUser)
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let preview = scans
            .preview(&PreviewInput {
                task_id: scan.id,
                cursor: None,
                limit: 100,
                include_adult: false,
            })
            .await
            .unwrap();
        scans
            .confirm(&ConfirmInput {
                task_id: scan.id,
                candidate_ids: preview.items.iter().map(|c| c.id).collect(),
            })
            .await
            .unwrap();
        let before = browse
            .list_media(&BrowseQuery {
                root_id: Some(root.root.id),
                kind: Some(MediaKind::TvSeries),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(
            before.items.len(),
            2,
            "both local series are visible before enrichment"
        );
        assert!(before
            .items
            .iter()
            .all(|item| item.health.contains(&"incomplete_metadata".to_owned())));
        let series = browse
            .tools_list_media(&BrowseQuery {
                kind: Some(MediaKind::TvSeries),
                root_id: Some(root.root.id),
                ..Default::default()
            })
            .await
            .unwrap()
            .items
            .into_iter()
            .find(|i| i.media.title == "Example Show")
            .unwrap()
            .media;
        let search = s
            .service
            .search(ScraperSearchRequest {
                query: VideoQuery::Tv {
                    title: "Example Show".into(),
                    year: Some(2020),
                    external_ids: vec![],
                    language: "en-US".into(),
                },
                providers: vec!["tmdb".into()],
            })
            .await
            .unwrap();
        assert!(matches!(
            s.service.detail(search.run_id, 0).await.unwrap(),
            VideoMetadata::Tv(_)
        ));
        let mut selected = selection(search.run_id, series.id);
        selected.selected_fields.push("genres".into());
        let plan = s.service.preview(selected).await.unwrap();
        s.service.confirm(plan.preview_id).await.unwrap();
        let query = BrowseQuery {
            root_id: Some(root.root.id),
            kind: Some(MediaKind::TvSeries),
            ..Default::default()
        };
        let ready = browse.list_media(&query).await.unwrap();
        assert_eq!(ready.items.len(), 2);
        let confirmed = ready
            .items
            .iter()
            .find(|item| item.media.id == series.id)
            .unwrap();
        assert_eq!(confirmed.media.title, "Confirmed show");
        assert!(!confirmed.health.contains(&"incomplete_metadata".to_owned()));
        let other = ready
            .items
            .iter()
            .find(|item| item.media.title == "Other Show")
            .unwrap();
        assert!(
            other.health.contains(&"incomplete_metadata".to_owned()),
            "metadata evidence does not transfer to unrelated local series"
        );
        assert_eq!(
            browse.facets(&query).await.unwrap().genres[0].value,
            "Drama"
        );
        let detail = browse
            .get_media(&MediaDetailInput {
                media_id: series.id,
                include_adult: false,
            })
            .await
            .unwrap();
        assert_eq!(
            detail.files.len(),
            2,
            "series exposes exact explicitly selectable local episodes"
        );
        struct StubVideoOpener(std::sync::atomic::AtomicUsize);
        impl crate::video_open::NativeVideoOpener for StubVideoOpener {
            fn open(&self, path: &std::path::Path) -> std::io::Result<()> {
                assert!(path.is_file());
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }
        let opener = Arc::new(StubVideoOpener(std::sync::atomic::AtomicUsize::new(0)));
        let native =
            crate::video_open::VideoOpenService::with_opener(s.repo.clone(), opener.clone());
        for file in &detail.files {
            assert!(native.open(file.id).await.unwrap().requested);
        }
        assert_eq!(opener.0.load(Ordering::SeqCst), 2);
        for file in &detail.files {
            assert!(browse
                .get_media(&MediaDetailInput {
                    media_id: file.media_item_id,
                    include_adult: false
                })
                .await
                .is_ok());
        }
        // A confirmed parent associated with an adult Root cannot authorize an
        // ordinary episode, even when a legacy low-level update omitted its marker.
        let adult_path = s.dir.path().join("private-root");
        std::fs::create_dir(&adult_path).unwrap();
        let adult_root = LibraryRoot {
            id: Uuid::new_v4(),
            name: "Private".into(),
            path: adult_path.to_str().unwrap().into(),
            path_style: crate::test_support::native_path_style(),
            kind: Some(MediaKind::AdultVideo),
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        s.repo.save_root(&adult_root).await.unwrap();
        let private_file = MediaFile {
            id: Uuid::new_v4(),
            media_item_id: series.id,
            root_id: adult_root.id,
            relative_path: "private.nfo".into(),
            size_bytes: 0,
            modified_at: None,
            part: None,
        };
        s.repo.save_file(&private_file).await.unwrap();
        // Even independently saved leaf metadata cannot expose an adult-associated
        // hierarchy through ordinary consumption.
        sqlx::query("INSERT INTO local_metadata_provenance(media_item_id,source_kind,source_reference,confirmed_at) VALUES(?,'nfo','previously confirmed leaf fixture',0)").bind(detail.files[0].media_item_id.to_string()).execute(s.repo.pool()).await.unwrap();
        for file in &detail.files {
            assert!(browse
                .get_media(&MediaDetailInput {
                    media_id: file.media_item_id,
                    include_adult: false
                })
                .await
                .is_err());
            assert!(native.open(file.id).await.is_err());
        }
        sqlx::query("DELETE FROM local_metadata_provenance WHERE media_item_id=?")
            .bind(detail.files[0].media_item_id.to_string())
            .execute(s.repo.pool())
            .await
            .unwrap();
        sqlx::query("DELETE FROM media_files WHERE id=?")
            .bind(private_file.id.to_string())
            .execute(s.repo.pool())
            .await
            .unwrap();
        // Restore this fixture's ordinary classification explicitly: removing a
        // private association intentionally does not clear the isolation marker.
        sqlx::query("UPDATE media_items SET adult_isolated=0 WHERE id=?")
            .bind(series.id.to_string())
            .execute(s.repo.pool())
            .await
            .unwrap();
        // Intermediate Season scope is checked too, not just source Series and leaf.
        let season: String = sqlx::query_scalar(
            "SELECT parent_id FROM media_relations WHERE child_id=? AND relation_type='episode'",
        )
        .bind(detail.files[0].media_item_id.to_string())
        .fetch_one(s.repo.pool())
        .await
        .unwrap();
        sqlx::query("UPDATE media_items SET adult_isolated=1 WHERE id=?")
            .bind(&season)
            .execute(s.repo.pool())
            .await
            .unwrap();
        assert!(browse
            .get_media(&MediaDetailInput {
                media_id: detail.files[0].media_item_id,
                include_adult: false
            })
            .await
            .is_err());
        assert!(native.open(detail.files[0].id).await.is_err());
        sqlx::query("UPDATE media_items SET adult_isolated=0 WHERE id=?")
            .bind(&season)
            .execute(s.repo.pool())
            .await
            .unwrap();
        // An arbitrary relation cannot transfer provider completion to a different work.
        sqlx::query("UPDATE media_relations SET relation_type='anime_base' WHERE parent_id=?")
            .bind(series.id.to_string())
            .execute(s.repo.pool())
            .await
            .unwrap();
        assert!(browse
            .list_media(&query)
            .await
            .unwrap()
            .items
            .iter()
            .all(|item| item.media.id != series.id));
        for file in &detail.files {
            let leaf = browse
                .get_media(&MediaDetailInput {
                    media_id: file.media_item_id,
                    include_adult: false,
                })
                .await
                .unwrap();
            assert!(leaf.health.contains(&"incomplete_metadata".to_owned()), "arbitrary parent relations do not transfer metadata to independently available local episodes");
            assert!(native.open(file.id).await.unwrap().requested);
        }
        s.server.abort();
    }
}
