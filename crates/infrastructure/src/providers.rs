//! Provider registry and persistent execution. The only bundled adapter is an explicit
//! offline fixture; enabling it never starts a request or modifies library metadata.
use crate::{
    library::{LibraryRepository, RepositoryError},
    network::{parse_html, parse_json, CancellationToken, NetworkEngine},
};
use chrono::{DateTime, Utc};
use mediaatlas_domain::{
    MediaKind, NetworkRequestRecord, ProviderConfig, ProviderDescriptor, ProviderFailure,
    ProviderInfo, ProviderJob, ProviderSnapshot, ProviderTaskView, ProviderView, SearchProvider,
};
use sqlx::Row;
use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc, time::Duration};
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

pub const FIXTURE_PROVIDER_ID: &str = "fixture.offline";

#[derive(Clone)]
pub struct ProviderService {
    repo: LibraryRepository,
    gate: Arc<Mutex<()>>,
    cancellations: Arc<Mutex<HashMap<Uuid, CancellationToken>>>,
    adapters: Arc<RwLock<HashMap<String, Arc<dyn ProviderAdapter>>>>,
    engine: Arc<NetworkEngine>,
}

/// Backend adapters are registered by trusted application composition, never IPC.
/// Each receives the immutable job, shared bounded HTTP engine and cancellation.
pub trait ProviderAdapter: Send + Sync {
    fn descriptor(&self) -> ProviderDescriptor;
    fn execute<'a>(
        &'a self,
        job: &'a ProviderJob,
        engine: &'a NetworkEngine,
        cancel: &'a CancellationToken,
    ) -> Pin<Box<dyn Future<Output = Result<String, ProviderFailure>> + Send + 'a>>;
}

/// Explicit immutable context for P3 ordinary-Movie jobs. Library/file association
/// is deliberately rejected until typed matching and merge arrive in P4.
#[derive(Debug, Clone)]
pub struct ProviderJobContext {
    pub media_kind: MediaKind,
    pub media_item_id: Option<Uuid>,
    pub file_identity: Option<String>,
    pub region: Option<String>,
    pub correlation_id: Option<Uuid>,
}
impl Default for ProviderJobContext {
    fn default() -> Self {
        Self {
            media_kind: MediaKind::Movie,
            media_item_id: None,
            file_identity: None,
            region: None,
            correlation_id: None,
        }
    }
}

/// Deliberately contains no remote URLs, credentials, or media-library mutations.
pub struct OfflineFixtureProvider;

impl ProviderInfo for OfflineFixtureProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor {
            id: FIXTURE_PROVIDER_ID.into(),
            name: "Offline fixture".into(),
            description:
                "Offline JSON and HTML parser contract fixture. No network or media writes.".into(),
            website: None,
            documentation_url: None,
            engine_types: vec!["api".into(), "html".into()],
            requires_api_key: false,
            requires_login: false,
            rate_limit_policy: "Offline parser operations only; no HTTP requests.".into(),
            attribution: "MediaAtlas synthetic fixture".into(),
            data_usage_notes: "Explicit development fixture only; never enriches library media."
                .into(),
            terms_url: None,
            privacy_url: None,
            license: "MIT (synthetic test data)".into(),
            media_kinds: vec![MediaKind::Movie],
            languages: vec!["zh-CN".into(), "en-US".into()],
            capabilities: vec!["search".into(), "api_json".into(), "http_html".into()],
            fixture: true,
        }
    }
}

impl SearchProvider<String, String> for OfflineFixtureProvider {
    async fn search(
        &self,
        _query: String,
        _job: &ProviderJob,
    ) -> Result<Vec<String>, ProviderFailure> {
        let api = parse_json(br#"{"id":"fixture-1","title":"Offline fixture"}"#)?;
        let html = parse_html(
            r#"<html><head><meta property="og:title" content="Offline fixture"><script type="application/ld+json">{"@type":"Movie","identifier":"fixture-1"}</script><script id="fixture-data" type="application/json">{"id":"fixture-1"}</script></head><body><h1>Offline fixture</h1></body></html>"#,
        );
        let title = html.css_text("h1")?;
        let structured = html.json_ld()?;
        let embedded = html.embedded_json("fixture-data")?;
        if api["id"] != embedded["id"]
            || structured.first().and_then(|v| v["identifier"].as_str()) != Some("fixture-1")
        {
            return Err(ProviderFailure::IdentityMismatch);
        }
        if title.first().map(String::as_str) != api["title"].as_str()
            || html.open_graph().get("og:title").map(String::as_str) != api["title"].as_str()
        {
            return Err(ProviderFailure::ParseError);
        }
        Ok(vec!["fixture_combined".into()])
    }
}

impl ProviderAdapter for OfflineFixtureProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderInfo::descriptor(self)
    }
    fn execute<'a>(
        &'a self,
        job: &'a ProviderJob,
        _engine: &'a NetworkEngine,
        cancel: &'a CancellationToken,
    ) -> Pin<Box<dyn Future<Output = Result<String, ProviderFailure>> + Send + 'a>> {
        Box::pin(async move {
            if job.config.max_response_bytes < 512 {
                return Err(ProviderFailure::ResponseTooLarge);
            }
            tokio::select! {
                _ = cancel.cancelled() => Err(ProviderFailure::Cancelled),
                _ = tokio::time::sleep(Duration::from_millis(job.config.interval_ms.max(100))) => {
                    self.search(job.query.clone(), job).await.map(|_|"fixture_combined".into())
                }
            }
        })
    }
}

impl ProviderService {
    pub async fn new(repo: LibraryRepository) -> Result<Self, RepositoryError> {
        let engine = NetworkEngine::new(4).map_err(|_| RepositoryError::InvalidInput)?;
        Self::with_engine(repo, engine).await
    }

    pub async fn with_engine(
        repo: LibraryRepository,
        engine: NetworkEngine,
    ) -> Result<Self, RepositoryError> {
        let service = Self {
            repo,
            gate: Arc::new(Mutex::new(())),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
            adapters: Arc::new(RwLock::new(HashMap::new())),
            engine: Arc::new(engine),
        };
        service
            .register_adapter(Arc::new(OfflineFixtureProvider))
            .await?;
        service.recover_interrupted().await?;
        Ok(service)
    }

    pub async fn register_adapter(
        &self,
        adapter: Arc<dyn ProviderAdapter>,
    ) -> Result<(), RepositoryError> {
        let _gate = self.gate.lock().await;
        let descriptor = adapter.descriptor();
        if descriptor.id.is_empty()
            || descriptor.id.len() > 80
            || !descriptor
                .id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        {
            return Err(RepositoryError::InvalidInput);
        }
        // P3 has no adult/context selector. Keep adapters on the ordinary Movie
        // lane until the later typed provider workflow adds explicit isolation.
        if !descriptor.media_kinds.contains(&MediaKind::Movie)
            || descriptor.media_kinds.contains(&MediaKind::AdultVideo)
        {
            return Err(RepositoryError::InvalidInput);
        }
        let mut adapters = self.adapters.write().await;
        if adapters.contains_key(&descriptor.id) {
            return Err(RepositoryError::Conflict);
        }
        sqlx::query("INSERT OR IGNORE INTO provider_configs(provider_id,config_json) VALUES (?,?)")
            .bind(&descriptor.id)
            .bind(encode(&ProviderConfig::default())?)
            .execute(self.repo.pool())
            .await?;
        adapters.insert(descriptor.id.clone(), adapter);
        Ok(())
    }

    pub async fn recover_interrupted(&self) -> Result<u64, RepositoryError> {
        let _gate = self.gate.lock().await;
        let now = Utc::now().timestamp_millis();
        let mut tx = self.repo.pool().begin().await?;
        let result = sqlx::query("UPDATE scrape_tasks SET state='interrupted',updated_at=MAX(updated_at,?) WHERE id IN (SELECT task_id FROM provider_sessions) AND state IN ('queued','running','waiting_for_user')")
            .bind(now).execute(&mut *tx).await?;
        sqlx::query("UPDATE scrape_runs SET state='interrupted',finished_at=MAX(started_at,?) WHERE task_id IN (SELECT task_id FROM provider_sessions) AND state IN ('queued','running','waiting_for_user')")
            .bind(now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(result.rows_affected())
    }

    async fn config(&self, provider_id: &str) -> Result<ProviderConfig, RepositoryError> {
        if !self.adapters.read().await.contains_key(provider_id) {
            return Err(RepositoryError::NotFound);
        }
        let value: String =
            sqlx::query_scalar("SELECT config_json FROM provider_configs WHERE provider_id=?")
                .bind(provider_id)
                .fetch_one(self.repo.pool())
                .await?;
        decode(&value)
    }

    pub async fn snapshot(&self) -> Result<ProviderSnapshot, RepositoryError> {
        let descriptors: Vec<_> = self
            .adapters
            .read()
            .await
            .values()
            .map(|a| a.descriptor())
            .collect();
        let mut providers = Vec::new();
        for descriptor in descriptors {
            let config = self.config(&descriptor.id).await?;
            let health = if config.enabled {
                self.engine.health(&descriptor.id).await
            } else {
                "disabled".into()
            };
            providers.push(ProviderView {
                descriptor,
                config,
                health,
            });
        }
        providers.sort_by(|a, b| a.descriptor.id.cmp(&b.descriptor.id));
        let rows = sqlx::query("SELECT t.id,t.state,t.created_at,t.updated_at,p.provider_id,p.job_json,p.failure,p.result_summary FROM provider_sessions p JOIN scrape_tasks t ON t.id=p.task_id ORDER BY t.created_at DESC,t.id DESC LIMIT 100")
            .fetch_all(self.repo.pool()).await?;
        let mut tasks = Vec::with_capacity(rows.len());
        for row in rows {
            let job: ProviderJob = decode(&row.try_get::<String, _>("job_json")?)?;
            tasks.push(ProviderTaskView {
                id: Uuid::parse_str(&row.try_get::<String, _>("id")?)
                    .map_err(|_| RepositoryError::CorruptData)?,
                provider_id: row.try_get("provider_id")?,
                state: decode(&format!("\"{}\"", row.try_get::<String, _>("state")?))?,
                query: job.query,
                created_at: timestamp(row.try_get("created_at")?)?,
                updated_at: timestamp(row.try_get("updated_at")?)?,
                failure: row
                    .try_get::<Option<String>, _>("failure")?
                    .map(|s| decode(&s))
                    .transpose()?,
                result_summary: row.try_get("result_summary")?,
            });
        }
        let requests = sqlx::query_scalar::<_, String>(
            "SELECT record_json FROM provider_requests ORDER BY id DESC LIMIT 100",
        )
        .fetch_all(self.repo.pool())
        .await?
        .iter()
        .map(|v| decode(v))
        .collect::<Result<Vec<_>, _>>()?;
        Ok(ProviderSnapshot {
            schema_version: 1,
            providers,
            tasks,
            requests,
        })
    }

    pub async fn set_enabled(
        &self,
        provider_id: &str,
        enabled: bool,
    ) -> Result<(), RepositoryError> {
        let _gate = self.gate.lock().await;
        let mut config = self.config(provider_id).await?;
        config.enabled = enabled;
        self.save_config(provider_id, config).await
    }

    /// Saves a new revision. Existing jobs retain their original config_json.
    pub async fn update_config(
        &self,
        provider_id: &str,
        config: &ProviderConfig,
    ) -> Result<(), RepositoryError> {
        let _gate = self.gate.lock().await;
        self.save_config(provider_id, config.clone()).await
    }

    async fn save_config(
        &self,
        provider_id: &str,
        mut config: ProviderConfig,
    ) -> Result<(), RepositoryError> {
        validate_config(&config)?;
        if provider_id == FIXTURE_PROVIDER_ID && config.credential_ref.is_some() {
            return Err(RepositoryError::InvalidInput);
        }
        let old = self.config(provider_id).await?;
        config.revision = old
            .revision
            .checked_add(1)
            .ok_or(RepositoryError::InvalidInput)?;
        sqlx::query("UPDATE provider_configs SET config_json=? WHERE provider_id=?")
            .bind(encode(&config)?)
            .bind(provider_id)
            .execute(self.repo.pool())
            .await?;
        if !config.enabled {
            let ids = sqlx::query_scalar::<_, String>("SELECT t.id FROM scrape_tasks t JOIN provider_sessions p ON p.task_id=t.id WHERE p.provider_id=? AND t.state IN ('queued','running')")
                .bind(provider_id).fetch_all(self.repo.pool()).await?;
            for id in ids {
                let id = Uuid::parse_str(&id).map_err(|_| RepositoryError::CorruptData)?;
                self.cancel_inner(id).await?;
            }
        }
        Ok(())
    }

    pub async fn start(&self, provider_id: &str, query: &str) -> Result<Uuid, RepositoryError> {
        self.start_localized(provider_id, query, "en-US").await
    }

    pub async fn start_localized(
        &self,
        provider_id: &str,
        query: &str,
        language: &str,
    ) -> Result<Uuid, RepositoryError> {
        self.start_context(provider_id, query, language, &ProviderJobContext::default())
            .await
    }

    pub async fn start_context(
        &self,
        provider_id: &str,
        query: &str,
        language: &str,
        context: &ProviderJobContext,
    ) -> Result<Uuid, RepositoryError> {
        let _gate = self.gate.lock().await;
        if !matches!(language, "zh-CN" | "en-US")
            || context.media_kind != MediaKind::Movie
            || context.media_item_id.is_some()
            || context.file_identity.is_some()
            || context.correlation_id.is_some_and(|id| id.is_nil())
            || context.region.as_ref().is_some_and(|region| {
                region.len() != 2 || !region.bytes().all(|b| b.is_ascii_uppercase())
            })
        {
            return Err(RepositoryError::InvalidInput);
        }
        let adapter = self
            .adapters
            .read()
            .await
            .get(provider_id)
            .cloned()
            .ok_or(RepositoryError::NotFound)?;
        if !adapter.descriptor().languages.iter().any(|s| s == language) {
            return Err(RepositoryError::InvalidInput);
        }
        validate_query(query)?;
        let config = self.config(provider_id).await?;
        if !config.enabled {
            return Err(RepositoryError::InvalidInput);
        }
        let active: i64 = sqlx::query_scalar("SELECT count(*) FROM provider_sessions p JOIN scrape_tasks t ON t.id=p.task_id WHERE t.state IN ('queued','running')")
            .fetch_one(self.repo.pool()).await?;
        if active >= 100 {
            return Err(RepositoryError::Conflict);
        }
        let id = Uuid::new_v4();
        let now = Utc::now();
        let job = ProviderJob {
            id,
            correlation_id: context.correlation_id.unwrap_or(id),
            media_kind: Some(context.media_kind),
            file_identity: context.file_identity.clone(),
            region: context.region.clone(),
            provider_id: provider_id.into(),
            media_item_id: context.media_item_id,
            operation: "search".into(),
            query: query.trim().into(),
            language: language.into(),
            config,
            created_at: now,
        };
        let mut tx = self.repo.pool().begin().await?;
        sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES (?,'search_providers','queued',?,?,?)")
            .bind(id.to_string()).bind(encode(&job)?).bind(now.timestamp_millis()).bind(now.timestamp_millis()).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO scrape_runs(id,task_id,provider_id,config_snapshot,state,started_at) VALUES (?,?,?,?,'queued',?)")
            .bind(id.to_string()).bind(id.to_string()).bind(provider_id).bind(encode(&job.config)?).bind(now.timestamp_millis()).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO provider_sessions(task_id,provider_id,job_json) VALUES (?,?,?)")
            .bind(id.to_string())
            .bind(provider_id)
            .bind(encode(&job)?)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        let cancel = CancellationToken::new();
        self.cancellations.lock().await.insert(id, cancel.clone());
        let service = self.clone();
        tokio::spawn(async move {
            if service.run(job.clone(), cancel).await.is_err() {
                let _ = service
                    .finish(&job, Err(ProviderFailure::NetworkError), &[])
                    .await;
            }
            service.cancellations.lock().await.remove(&id);
        });
        Ok(id)
    }

    pub async fn cancel(&self, task_id: Uuid) -> Result<bool, RepositoryError> {
        let _gate = self.gate.lock().await;
        self.cancel_inner(task_id).await
    }

    async fn cancel_inner(&self, task_id: Uuid) -> Result<bool, RepositoryError> {
        let mut tx = self.repo.pool().begin().await?;
        let result = sqlx::query("UPDATE scrape_tasks SET state='cancelled',updated_at=MAX(updated_at,?) WHERE id=? AND id IN (SELECT task_id FROM provider_sessions) AND state IN ('queued','running')")
            .bind(Utc::now().timestamp_millis()).bind(task_id.to_string()).execute(&mut *tx).await?;
        if result.rows_affected() == 1 {
            sqlx::query("UPDATE provider_sessions SET failure=? WHERE task_id=?")
                .bind(encode(&ProviderFailure::Cancelled)?)
                .bind(task_id.to_string())
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE scrape_runs SET state='cancelled',finished_at=MAX(started_at,?) WHERE task_id=? AND state IN ('queued','running')")
                .bind(Utc::now().timestamp_millis()).bind(task_id.to_string()).execute(&mut *tx).await?;
        }
        tx.commit().await?;
        if result.rows_affected() == 1 {
            if let Some(cancel) = self.cancellations.lock().await.get(&task_id) {
                cancel.cancel();
            }
        }
        Ok(result.rows_affected() == 1)
    }

    async fn run(
        &self,
        job: ProviderJob,
        cancel: CancellationToken,
    ) -> Result<(), RepositoryError> {
        let adapter = self
            .adapters
            .read()
            .await
            .get(&job.provider_id)
            .cloned()
            .ok_or(RepositoryError::NotFound)?;
        if cancel.is_cancelled() {
            return Ok(());
        }
        let updated = sqlx::query("UPDATE scrape_tasks SET state='running',updated_at=MAX(updated_at,?) WHERE id=? AND state='queued'")
            .bind(Utc::now().timestamp_millis()).bind(job.id.to_string()).execute(self.repo.pool()).await?;
        if updated.rows_affected() == 0 {
            return Ok(());
        }
        sqlx::query("UPDATE scrape_runs SET state='running' WHERE task_id=? AND state='queued'")
            .bind(job.id.to_string())
            .execute(self.repo.pool())
            .await?;
        let started = std::time::Instant::now();
        // The adapter must forward cancellation into NetworkEngine. Let the engine
        // finish its cancellation record before gathering persistent history.
        let result = tokio::time::timeout(
            Duration::from_millis(job.config.timeout_ms.saturating_add(100)),
            adapter.execute(&job, &self.engine, &cancel),
        )
        .await
        .unwrap_or(Err(ProviderFailure::Timeout));
        let mut records = if adapter.descriptor().fixture
            && matches!(
                &result,
                Err(ProviderFailure::Cancelled | ProviderFailure::Timeout)
            ) {
            Vec::new()
        } else if adapter.descriptor().fixture {
            ["api", "html"]
                .into_iter()
                .map(|engine| NetworkRequestRecord {
                    task_id: job.id,
                    provider_id: job.provider_id.clone(),
                    engine: engine.into(),
                    method: "FIXTURE".into(),
                    safe_url: "fixture://offline".into(),
                    summary: Some("fixture_parser".into()),
                    duration_ms: started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                    status: None,
                    failure: result.as_ref().err().copied(),
                    attempt: 1,
                })
                .collect::<Vec<_>>()
        } else {
            self.engine.take_records(job.id).await
        };
        if let Err(failure @ (ProviderFailure::ParseError | ProviderFailure::IdentityMismatch)) =
            &result
        {
            if let Some(record) = records.last_mut() {
                record.failure = Some(*failure);
                record.summary = Some("provider_validation_failed".into());
            }
        }
        self.finish(&job, result.map(|s| vec![s]), &records).await
    }

    async fn finish(
        &self,
        job: &ProviderJob,
        result: Result<Vec<String>, ProviderFailure>,
        records: &[NetworkRequestRecord],
    ) -> Result<(), RepositoryError> {
        let mut tx = self.repo.pool().begin().await?;
        let state = match &result {
            Ok(_) => "succeeded",
            Err(ProviderFailure::Cancelled) => "cancelled",
            Err(_) => "failed",
        };
        let updated = sqlx::query("UPDATE scrape_tasks SET state=?,updated_at=MAX(updated_at,?) WHERE id=? AND state IN ('queued','running')")
            .bind(state).bind(Utc::now().timestamp_millis()).bind(job.id.to_string()).execute(&mut *tx).await?;
        if updated.rows_affected() == 1 {
            sqlx::query("UPDATE provider_sessions SET failure=?,result_summary=? WHERE task_id=?")
                .bind(result.as_ref().err().map(encode).transpose()?)
                .bind(result.as_ref().ok().and_then(|v| v.first()))
                .bind(job.id.to_string())
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "UPDATE scrape_runs SET state=?,finished_at=MAX(started_at,?) WHERE task_id=?",
            )
            .bind(state)
            .bind(Utc::now().timestamp_millis())
            .bind(job.id.to_string())
            .execute(&mut *tx)
            .await?;
        }
        for record in records {
            sqlx::query("INSERT INTO provider_requests(task_id,record_json) VALUES (?,?)")
                .bind(job.id.to_string())
                .bind(encode(record)?)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(())
    }
}

fn validate_query(query: &str) -> Result<(), RepositoryError> {
    let lower = query.to_ascii_lowercase();
    let compact: String = lower.chars().filter(|c| !c.is_whitespace()).collect();
    if query.trim().is_empty()
        || query.len() > 256
        || query.chars().any(char::is_control)
        || [
            "://",
            "token=",
            "secret=",
            "password=",
            "api_key=",
            "authorization:",
            "cookie:",
            "token:",
            "secret:",
            "password:",
            "apikey=",
            "apikey:",
            "api_key:",
            "access_token=",
            "bearer ",
        ]
        .iter()
        .any(|s| lower.contains(s) || compact.contains(s))
    {
        Err(RepositoryError::InvalidInput)
    } else {
        Ok(())
    }
}
fn validate_config(config: &ProviderConfig) -> Result<(), RepositoryError> {
    // Only opaque reference identifiers may be persisted; actual credentials are
    // resolved by a trusted adapter at execution time and never enter this DTO.
    // The offline fixture additionally rejects all references in save_config.
    if !(1..=120_000).contains(&config.timeout_ms)
        || !(1..=32).contains(&config.concurrency)
        || !(1..=16).contains(&config.image_concurrency)
        || config.interval_ms > 60000
        || !(1..=16_777_216).contains(&config.max_response_bytes)
        || config.max_retries > 3
        || config.credential_ref.as_ref().is_some_and(|r| {
            r.is_empty()
                || r.len() > 128
                || !r
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
    {
        Err(RepositoryError::InvalidInput)
    } else {
        Ok(())
    }
}
fn timestamp(value: i64) -> Result<DateTime<Utc>, RepositoryError> {
    DateTime::from_timestamp_millis(value).ok_or(RepositoryError::CorruptData)
}
fn encode<T: serde::Serialize>(value: &T) -> Result<String, RepositoryError> {
    serde_json::to_string(value).map_err(|_| RepositoryError::InvalidInput)
}
fn decode<T: serde::de::DeserializeOwned>(value: &str) -> Result<T, RepositoryError> {
    serde_json::from_str(value).map_err(|_| RepositoryError::CorruptData)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::{EngineKind, RequestSpec};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    struct MockAdapter {
        id: String,
        url: String,
    }
    impl ProviderAdapter for MockAdapter {
        fn descriptor(&self) -> ProviderDescriptor {
            let mut descriptor = ProviderInfo::descriptor(&OfflineFixtureProvider);
            descriptor.id = self.id.clone();
            descriptor.fixture = false;
            descriptor
        }
        fn execute<'a>(
            &'a self,
            job: &'a ProviderJob,
            engine: &'a NetworkEngine,
            cancel: &'a CancellationToken,
        ) -> Pin<Box<dyn Future<Output = Result<String, ProviderFailure>> + Send + 'a>> {
            Box::pin(async move {
                let response = engine
                    .execute(job, RequestSpec::get(&self.url, EngineKind::Api), cancel)
                    .await?;
                if response.json()?["id"] != "fixture-1" {
                    return Err(ProviderFailure::IdentityMismatch);
                }
                Ok("fixture_combined".into())
            })
        }
    }
    async fn server(status: &str, delay: Duration) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let status = status.to_owned();
        let task = tokio::spawn(async move {
            loop {
                let Ok((mut stream, _)) = listener.accept().await else {
                    return;
                };
                let status = status.clone();
                tokio::spawn(async move {
                    let mut request = [0u8; 2048];
                    let _ = stream.read(&mut request).await;
                    tokio::time::sleep(delay).await;
                    let body = r#"{"id":"fixture-1"}"#;
                    let response=format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nRetry-After: 1\r\nConnection: close\r\n\r\n{body}",body.len());
                    let _ = stream.write_all(response.as_bytes()).await;
                });
            }
        });
        (format!("http://{addr}/fixture?token=do-not-log"), task)
    }
    async fn wait(service: &ProviderService, id: Uuid) -> ProviderTaskView {
        for _ in 0..200 {
            let task = service
                .snapshot()
                .await
                .unwrap()
                .tasks
                .into_iter()
                .find(|t| t.id == id)
                .unwrap();
            if !matches!(
                task.state,
                mediaatlas_domain::TaskState::Queued | mediaatlas_domain::TaskState::Running
            ) {
                return task;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("mock adapter did not finish");
    }
    async fn configured(service: &ProviderService, id: &str, timeout_ms: u64) {
        service
            .update_config(
                id,
                &ProviderConfig {
                    enabled: true,
                    interval_ms: 0,
                    max_retries: 0,
                    timeout_ms,
                    ..Default::default()
                },
            )
            .await
            .unwrap();
    }

    struct AdultAdapter;
    impl ProviderAdapter for AdultAdapter {
        fn descriptor(&self) -> ProviderDescriptor {
            let mut d = ProviderInfo::descriptor(&OfflineFixtureProvider);
            d.id = "fixture.adult".into();
            d.media_kinds = vec![MediaKind::Movie, MediaKind::AdultVideo];
            d
        }
        fn execute<'a>(
            &'a self,
            _job: &'a ProviderJob,
            _engine: &'a NetworkEngine,
            _cancel: &'a CancellationToken,
        ) -> Pin<Box<dyn Future<Output = Result<String, ProviderFailure>> + Send + 'a>> {
            Box::pin(async { Ok("never".into()) })
        }
    }
    #[tokio::test]
    async fn registry_rejects_adult_adapters_until_typed_adult_workflow_exists() {
        let dir = tempfile::tempdir().unwrap();
        let repo = LibraryRepository::open(&dir.path().join("runtime.sqlite"))
            .await
            .unwrap();
        let service = ProviderService::with_engine(repo, NetworkEngine::for_loopback(1))
            .await
            .unwrap();
        assert_eq!(
            service.register_adapter(Arc::new(AdultAdapter)).await,
            Err(RepositoryError::InvalidInput)
        );
        assert_eq!(service.snapshot().await.unwrap().providers.len(), 1);
    }

    #[tokio::test]
    async fn registered_adapters_execute_shared_engine_and_persist_failure_health_and_isolation() {
        let dir = tempfile::tempdir().unwrap();
        let repo = LibraryRepository::open(&dir.path().join("runtime.sqlite"))
            .await
            .unwrap();
        let service = ProviderService::with_engine(repo.clone(), NetworkEngine::for_loopback(4))
            .await
            .unwrap();
        let (bad_url, bad_server) = server("429 Too Many Requests", Duration::ZERO).await;
        let (good_url, good_server) = server("200 OK", Duration::from_millis(10)).await;
        service
            .register_adapter(Arc::new(MockAdapter {
                id: "mock.blocked".into(),
                url: bad_url,
            }))
            .await
            .unwrap();
        service
            .register_adapter(Arc::new(MockAdapter {
                id: "mock.healthy".into(),
                url: good_url,
            }))
            .await
            .unwrap();
        configured(&service, "mock.blocked", 2000).await;
        configured(&service, "mock.healthy", 2000).await;
        let bad = service.start("mock.blocked", "First query").await.unwrap();
        let good = service.start("mock.healthy", "Second query").await.unwrap();
        assert_eq!(
            wait(&service, bad).await.failure,
            Some(ProviderFailure::RateLimited)
        );
        assert_eq!(
            wait(&service, good).await.state,
            mediaatlas_domain::TaskState::Succeeded
        );
        let snapshot = service.snapshot().await.unwrap();
        assert_eq!(
            snapshot
                .providers
                .iter()
                .find(|p| p.descriptor.id == "mock.blocked")
                .unwrap()
                .health,
            "cooling_down"
        );
        assert_eq!(
            snapshot
                .providers
                .iter()
                .find(|p| p.descriptor.id == "mock.healthy")
                .unwrap()
                .health,
            "healthy"
        );
        assert!(snapshot.requests.iter().any(|r| r.task_id == bad
            && r.status == Some(429)
            && r.failure == Some(ProviderFailure::RateLimited)));
        assert!(snapshot
            .requests
            .iter()
            .any(|r| r.task_id == good && r.status == Some(200)));
        assert!(snapshot
            .requests
            .iter()
            .all(|r| !r.safe_url.contains("token")));
        let persisted: String = sqlx::query_scalar("SELECT state FROM scrape_runs WHERE task_id=?")
            .bind(bad.to_string())
            .fetch_one(repo.pool())
            .await
            .unwrap();
        assert_eq!(persisted, "failed");
        bad_server.abort();
        good_server.abort();
    }

    #[tokio::test]
    async fn running_adapter_cancel_and_config_changes_remain_per_job_and_persist() {
        let dir = tempfile::tempdir().unwrap();
        let repo = LibraryRepository::open(&dir.path().join("runtime.sqlite"))
            .await
            .unwrap();
        let service = ProviderService::with_engine(repo.clone(), NetworkEngine::for_loopback(4))
            .await
            .unwrap();
        let (url, server) = server("200 OK", Duration::from_millis(150)).await;
        service
            .register_adapter(Arc::new(MockAdapter {
                id: "mock.slow".into(),
                url,
            }))
            .await
            .unwrap();
        configured(&service, "mock.slow", 1000).await;
        let old = service.start("mock.slow", "Original query").await.unwrap();
        configured(&service, "mock.slow", 10).await;
        let new = service.start("mock.slow", "New query").await.unwrap();
        assert_eq!(
            wait(&service, new).await.failure,
            Some(ProviderFailure::Timeout)
        );
        assert_eq!(
            wait(&service, old).await.state,
            mediaatlas_domain::TaskState::Succeeded
        );
        configured(&service, "mock.slow", 1000).await;
        let cancelled = service.start("mock.slow", "Cancel query").await.unwrap();
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(service.cancel(cancelled).await.unwrap());
        assert_eq!(
            wait(&service, cancelled).await.state,
            mediaatlas_domain::TaskState::Cancelled
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            wait(&service, cancelled).await.failure,
            Some(ProviderFailure::Cancelled)
        );
        let saved: String = sqlx::query_scalar("SELECT state FROM scrape_runs WHERE task_id=?")
            .bind(cancelled.to_string())
            .fetch_one(repo.pool())
            .await
            .unwrap();
        assert_eq!(saved, "cancelled");
        assert!(service
            .snapshot()
            .await
            .unwrap()
            .requests
            .iter()
            .any(|r| r.task_id == cancelled && r.failure == Some(ProviderFailure::Cancelled)));
        server.abort();
    }
}
