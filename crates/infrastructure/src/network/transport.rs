use super::{
    parse_json,
    policy::{DestinationPolicy, SafeResolver},
    safe_origin, CancellationToken,
};
use mediaatlas_domain::{NetworkRequestRecord, ProviderConfig, ProviderFailure, ProviderJob};
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE, LOCATION, RETRY_AFTER},
    Client, Method, Url,
};
use serde_json::Value;
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::{Mutex, Notify, Semaphore};

/// Intentionally neither Serialize nor a printable credential value.
#[derive(Clone)]
pub struct SecretString(String);
impl SecretString {
    pub fn new(value: String) -> Self {
        Self(value)
    }
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for SecretString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretString([REDACTED])")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineKind {
    Api,
    Html,
    Image,
}
impl EngineKind {
    fn name(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::Html => "html",
            Self::Image => "image",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestMethod {
    Get,
    Post,
}
impl RequestMethod {
    fn name(self) -> &'static str {
        match self {
            Self::Get => "GET",
            Self::Post => "POST",
        }
    }
}
#[derive(Clone)]
pub struct RequestSpec {
    pub url: String,
    pub method: RequestMethod,
    pub engine: EngineKind,
    pub json_body: Option<Value>,
    pub bearer: Option<SecretString>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub cookie: Option<SecretString>,
    /// API-key header name and secret. Forbidden routing/header names are rejected.
    pub api_key: Option<(String, SecretString)>,
    /// Added only to the actual original-origin request; never to stored URLs.
    pub secret_query: Option<(String, SecretString)>,
    /// Multiple inbound-only query credential slots, validated and origin-confined.
    pub secret_queries: Vec<(String, SecretString)>,
    pub expected_identity: Option<(String, String)>,
}
impl RequestSpec {
    pub fn get(url: impl Into<String>, engine: EngineKind) -> Self {
        Self {
            url: url.into(),
            method: RequestMethod::Get,
            engine,
            json_body: None,
            bearer: None,
            user_agent: None,
            referer: None,
            cookie: None,
            api_key: None,
            secret_query: None,
            secret_queries: Vec::new(),
            expected_identity: None,
        }
    }
    pub fn graphql(
        url: impl Into<String>,
        query: &str,
        variables: Value,
    ) -> Result<Self, ProviderFailure> {
        if !(query.trim_start().starts_with("query") || query.trim_start().starts_with('{'))
            || !variables.is_object()
        {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        let mut spec = Self::get(url, EngineKind::Api);
        spec.method = RequestMethod::Post;
        spec.json_body = Some(serde_json::json!({"query":query,"variables":variables}));
        Ok(spec)
    }
}
pub struct EngineResponse {
    pub body: Vec<u8>,
    pub status: u16,
    pub content_type: Option<String>,
    pub record: NetworkRequestRecord,
}
impl EngineResponse {
    pub fn json(&self) -> Result<Value, ProviderFailure> {
        parse_json(&self.body)
    }
    pub fn text(&self) -> Result<&str, ProviderFailure> {
        std::str::from_utf8(&self.body).map_err(|_| ProviderFailure::ParseError)
    }
}

#[derive(Default)]
struct SiteState {
    active: usize,
    images: usize,
    next_allowed: Option<Instant>,
    cooldown: Option<Instant>,
    failures: u32,
    probe: bool,
    unhealthy: bool,
    blocked_indefinitely: bool,
}
#[derive(Default)]
struct Site {
    state: Mutex<SiteState>,
    notify: Notify,
}
struct SitePermit {
    site: Arc<Site>,
    image: bool,
    probe: bool,
}
impl Drop for SitePermit {
    fn drop(&mut self) {
        let site = self.site.clone();
        let image = self.image;
        let probe = self.probe;
        tokio::spawn(async move {
            let mut s = site.state.lock().await;
            s.active -= 1;
            if probe {
                s.probe = false;
            }
            if image {
                s.images -= 1;
            }
            drop(s);
            site.notify.notify_waiters();
        });
    }
}
#[derive(Clone)]
pub struct NetworkEngine {
    client: Client,
    policy: Arc<DestinationPolicy>,
    global: Arc<Semaphore>,
    images: Arc<Semaphore>,
    sites: Arc<Mutex<HashMap<String, Arc<Site>>>>,
    records: Arc<Mutex<HashMap<uuid::Uuid, VecDeque<NetworkRequestRecord>>>>,
    inherited_untrusted_proxy: bool,
    trusted_proxy: bool,
}
struct AttemptFailure {
    kind: ProviderFailure,
    status: Option<u16>,
    retry_after: Option<Duration>,
}
impl From<ProviderFailure> for AttemptFailure {
    fn from(kind: ProviderFailure) -> Self {
        Self {
            kind,
            status: None,
            retry_after: None,
        }
    }
}

impl NetworkEngine {
    pub fn new(global_limit: usize) -> Result<Self, ProviderFailure> {
        Self::build(global_limit, DestinationPolicy::default(), None)
    }
    /// Configure independent global HTTP and image budgets (shared across sites).
    pub fn with_limits(global_limit: usize, image_limit: usize) -> Result<Self, ProviderFailure> {
        if image_limit == 0 || image_limit > 64 {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        let mut engine = Self::new(global_limit)?;
        engine.images = Arc::new(Semaphore::new(image_limit));
        Ok(engine)
    }
    /// The gateway operator MUST enforce private-address restrictions at its own
    /// DNS resolution/CONNECT boundary. Local preflight alone cannot prevent a
    /// proxy resolving a different address. This opt-in is never exposed to IPC.
    pub fn with_trusted_proxy(
        global_limit: usize,
        proxy_url: &str,
    ) -> Result<Self, ProviderFailure> {
        let url = Url::parse(proxy_url).map_err(|_| ProviderFailure::InvalidConfiguration)?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        Self::build(global_limit, DestinationPolicy::default(), Some(proxy_url))
    }
    fn build(
        global_limit: usize,
        policy: DestinationPolicy,
        proxy: Option<&str>,
    ) -> Result<Self, ProviderFailure> {
        if global_limit == 0 || global_limit > 64 {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        let policy = Arc::new(policy);
        let inherited_untrusted_proxy = proxy.is_none()
            && [
                "HTTPS_PROXY",
                "https_proxy",
                "HTTP_PROXY",
                "http_proxy",
                "ALL_PROXY",
                "all_proxy",
            ]
            .iter()
            .any(|n| std::env::var(n).is_ok_and(|v| !v.is_empty()));
        let mut builder = Client::builder()
            .retry(reqwest::retry::never())
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(60))
            .pool_max_idle_per_host(4)
            .user_agent("MediaAtlas/0.1 ProviderEngine");
        if let Some(proxy) = proxy {
            builder = builder.proxy(
                reqwest::Proxy::all(proxy).map_err(|_| ProviderFailure::InvalidConfiguration)?,
            );
        } else {
            builder = builder.dns_resolver(Arc::new(SafeResolver(policy.clone())));
        }
        #[cfg(test)]
        if policy.loopback {
            builder = builder.no_proxy();
        }
        let client = builder
            .build()
            .map_err(|_| ProviderFailure::InvalidConfiguration)?;
        Ok(Self {
            client,
            policy,
            global: Arc::new(Semaphore::new(global_limit)),
            images: Arc::new(Semaphore::new(global_limit.min(4))),
            sites: Arc::default(),
            records: Arc::default(),
            inherited_untrusted_proxy,
            trusted_proxy: proxy.is_some(),
        })
    }
    #[cfg(test)]
    pub(crate) fn for_loopback(global_limit: usize) -> Self {
        let mut s = Self::build(global_limit, DestinationPolicy { loopback: true }, None).unwrap();
        s.inherited_untrusted_proxy = false;
        s
    }
    pub async fn records(&self) -> Vec<NetworkRequestRecord> {
        self.records
            .lock()
            .await
            .values()
            .flat_map(|r| r.iter().cloned())
            .collect()
    }
    pub async fn drain_records(&self) -> Vec<NetworkRequestRecord> {
        self.records
            .lock()
            .await
            .drain()
            .flat_map(|(_, r)| r)
            .collect()
    }
    pub async fn take_records(&self, task_id: uuid::Uuid) -> Vec<NetworkRequestRecord> {
        self.records
            .lock()
            .await
            .remove(&task_id)
            .map(|r| r.into_iter().collect())
            .unwrap_or_default()
    }
    #[cfg(test)]
    pub(crate) fn for_test_resolver<R: reqwest::dns::Resolve + 'static>(resolver: Arc<R>) -> Self {
        let mut s = Self::for_loopback(8);
        s.client = Client::builder()
            .retry(reqwest::retry::never())
            .no_proxy()
            .dns_resolver(resolver)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap();
        s
    }
    pub async fn health(&self, provider_id: &str) -> String {
        let Some(site) = self.sites.lock().await.get(provider_id).cloned() else {
            return "healthy".into();
        };
        let s = site.state.lock().await;
        if s.blocked_indefinitely || s.cooldown.is_some_and(|t| t > Instant::now()) {
            "cooling_down"
        } else if s.unhealthy {
            "probe_ready"
        } else {
            "healthy"
        }
        .into()
    }
    pub async fn execute_with_store(
        &self,
        job: &ProviderJob,
        mut spec: RequestSpec,
        cancel: &CancellationToken,
        store: &crate::credentials::CredentialStore,
        mode: crate::credentials::CredentialMode,
    ) -> Result<EngineResponse, ProviderFailure> {
        if let Some(reference) = &job.config.credential_ref {
            let started = Instant::now();
            let secret = tokio::select! {biased;_=cancel.cancelled()=>Err(ProviderFailure::Cancelled),r=tokio::time::timeout(Duration::from_millis(job.config.timeout_ms.max(1)),store.get(reference,mode))=>r.unwrap_or(Err(ProviderFailure::Timeout))};
            match secret {
                Ok(secret) => spec.bearer = Some(secret),
                Err(failure) => {
                    self.record(job, &spec, started, None, Some(failure), 0)
                        .await;
                    return Err(failure);
                }
            }
        }
        self.execute(job, spec, cancel).await
    }
    pub async fn execute(
        &self,
        job: &ProviderJob,
        spec: RequestSpec,
        cancel: &CancellationToken,
    ) -> Result<EngineResponse, ProviderFailure> {
        {
            let mut records = self.records.lock().await;
            if !records.contains_key(&job.id) && records.len() >= 100 {
                return Err(ProviderFailure::ResponseTooLarge);
            }
            records.entry(job.id).or_default();
        }
        // MusicBrainz policy is a site minimum, independent of saved UI limits.
        let mut bounded_job = job.clone();
        if matches!(job.provider_id.as_str(), "douban" | "bangumi" | "omdb") {
            let minimum = if job.provider_id == "douban" { 6000 } else { 1000 };
            bounded_job.config.interval_ms = bounded_job.config.interval_ms.max(minimum);
            bounded_job.config.concurrency = 1;
        }
        if job.provider_id == "musicbrainz"
            || Url::parse(&spec.url)
                .ok()
                .and_then(|u| u.host_str().map(str::to_owned))
                .as_deref()
                == Some("musicbrainz.org")
        {
            bounded_job.config.interval_ms = bounded_job.config.interval_ms.max(1000);
            bounded_job.config.concurrency = 1;
        }
        let job = &bounded_job;
        let start = Instant::now();
        let result = tokio::select! {
            biased;
            _=cancel.cancelled()=>Err(ProviderFailure::Cancelled),
            result=tokio::time::timeout(Duration::from_millis(job.config.timeout_ms.max(1)),self.execute_inner(job,&spec,cancel))=>result.unwrap_or(Err(ProviderFailure::Timeout)),
        };
        if let Err(kind) = result {
            // Preflight/queue/cancellation errors must be inspectable too. Attempts
            // already recorded are not duplicated, except overall timeouts.
            let last = self
                .records
                .lock()
                .await
                .get(&job.id)
                .and_then(|r| r.back())
                .cloned();
            if !last.is_some_and(|r| r.task_id == job.id && r.failure == Some(kind)) {
                self.record(job, &spec, start, None, Some(kind), 0).await;
            }
        }
        result
    }
    async fn execute_inner(
        &self,
        job: &ProviderJob,
        spec: &RequestSpec,
        cancel: &CancellationToken,
    ) -> Result<EngineResponse, ProviderFailure> {
        validate_config(&job.config)?;
        if spec.method == RequestMethod::Get && spec.json_body.is_some() {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        if !job.config.enabled {
            return Err(ProviderFailure::Disabled);
        }
        if self.inherited_untrusted_proxy {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        if let Some(referer) = &spec.referer {
            if referer.len() > 4096 {
                return Err(ProviderFailure::InvalidConfiguration);
            }
            self.policy.validate_url(
                &Url::parse(referer).map_err(|_| ProviderFailure::InvalidConfiguration)?,
            )?;
        }
        if spec
            .user_agent
            .as_ref()
            .is_some_and(|v| v.is_empty() || v.len() > 1024)
        {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        let url = Url::parse(&spec.url).map_err(|_| ProviderFailure::UnsafeTarget)?;
        self.policy.validate_url(&url)?;
        let site = {
            let mut sites = self.sites.lock().await;
            let key =
                if job.provider_id == "musicbrainz" || url.host_str() == Some("musicbrainz.org") {
                    "musicbrainz.org".to_owned()
                } else {
                    job.provider_id.clone()
                };
            sites.entry(key).or_default().clone()
        };
        let image = spec.engine == EngineKind::Image;
        let attempts = if spec.method == RequestMethod::Get {
            job.config.max_retries + 1
        } else {
            1
        };
        for attempt in 1..=attempts {
            let started = Instant::now();
            let result = loop {
                let site_permit = self
                    .acquire_site(site.clone(), &job.config, image, cancel)
                    .await?;
                let image_permit = if image {
                    Some(
                        tokio::select! {_=cancel.cancelled()=>return Err(ProviderFailure::Cancelled),p=self.images.acquire()=>p.map_err(|_|ProviderFailure::Cancelled)?},
                    )
                } else {
                    None
                };
                let global_permit = tokio::select! {_=cancel.cancelled()=>return Err(ProviderFailure::Cancelled),p=self.global.acquire()=>p.map_err(|_|ProviderFailure::Cancelled)?};
                let mut state = site.state.lock().await;
                if state.blocked_indefinitely || state.cooldown.is_some_and(|t| t > Instant::now())
                {
                    return Err(ProviderFailure::CoolingDown);
                }
                if let Some(wait) = state
                    .next_allowed
                    .and_then(|t| t.checked_duration_since(Instant::now()))
                {
                    drop(state);
                    drop(global_permit);
                    drop(image_permit);
                    drop(site_permit);
                    tokio::select! {_=cancel.cancelled()=>return Err(ProviderFailure::Cancelled),_=tokio::time::sleep(wait)=>{}}
                    continue;
                }
                state.next_allowed =
                    Instant::now().checked_add(Duration::from_millis(job.config.interval_ms));
                drop(state);
                break self.attempt(job, spec, cancel).await;
            };
            let (status, failure) = match &result {
                Ok((_, s, _)) => (*s, None),
                Err(e) => (e.status.unwrap_or_default(), Some(e.kind)),
            };
            let record = self
                .record(
                    job,
                    spec,
                    started,
                    (status != 0).then_some(status),
                    failure,
                    attempt,
                )
                .await;
            match result {
                Ok((body, status, content_type)) => {
                    let mut state = site.state.lock().await;
                    if !state.blocked_indefinitely
                        && state.cooldown.is_none_or(|t| t <= Instant::now())
                    {
                        state.failures = 0;
                        state.cooldown = None;
                        state.unhealthy = false;
                    }
                    return Ok(EngineResponse {
                        body,
                        status,
                        content_type,
                        record,
                    });
                }
                Err(error) => {
                    let retryable = matches!(
                        error.kind,
                        ProviderFailure::Timeout
                            | ProviderFailure::NetworkError
                            | ProviderFailure::DnsError
                            | ProviderFailure::Http5xx
                            | ProviderFailure::RateLimited
                    );
                    let mut wait = error
                        .retry_after
                        .unwrap_or(Duration::from_millis(100 * u64::from(attempt)));
                    {
                        let mut state = site.state.lock().await;
                        if retryable {
                            state.failures += 1;
                        }
                        if error.kind == ProviderFailure::RateLimited || state.failures >= 3 {
                            wait = wait.max(Duration::from_millis(500));
                            let until = Instant::now().checked_add(wait);
                            if let Some(until) = until {
                                state.cooldown =
                                    Some(state.cooldown.map_or(until, |old| old.max(until)));
                            } else {
                                state.blocked_indefinitely = true;
                            }
                            state.unhealthy = true;
                        }
                    }
                    if !retryable || attempt == attempts {
                        return Err(error.kind);
                    }
                    tokio::select! {_=cancel.cancelled()=>return Err(ProviderFailure::Cancelled),_=tokio::time::sleep(wait.min(Duration::from_secs(121)))=>{}}
                }
            }
        }
        Err(ProviderFailure::NetworkError)
    }
    async fn acquire_site(
        &self,
        site: Arc<Site>,
        config: &ProviderConfig,
        image: bool,
        cancel: &CancellationToken,
    ) -> Result<SitePermit, ProviderFailure> {
        loop {
            let notified = site.notify.notified();
            let mut s = site.state.lock().await;
            if s.blocked_indefinitely || s.cooldown.is_some_and(|t| t > Instant::now()) || s.probe {
                return Err(ProviderFailure::CoolingDown);
            }
            let wait = s
                .next_allowed
                .and_then(|t| t.checked_duration_since(Instant::now()));
            if s.active < config.concurrency
                && (!image || s.images < config.image_concurrency)
                && wait.is_none()
            {
                s.active += 1;
                if image {
                    s.images += 1;
                }
                let probe = s.unhealthy;
                if probe {
                    s.probe = true;
                }
                drop(s);
                return Ok(SitePermit {
                    site: site.clone(),
                    image,
                    probe,
                });
            }
            drop(s);
            tokio::select! {_=cancel.cancelled()=>return Err(ProviderFailure::Cancelled),_=async {if let Some(wait)=wait {tokio::time::sleep(wait).await;}else{notified.await;}}=>{}}
        }
    }
    async fn record(
        &self,
        job: &ProviderJob,
        spec: &RequestSpec,
        start: Instant,
        status: Option<u16>,
        failure: Option<ProviderFailure>,
        attempt: u8,
    ) -> NetworkRequestRecord {
        let record = NetworkRequestRecord {
            task_id: job.id,
            provider_id: job.provider_id.clone(),
            engine: spec.engine.name().into(),
            method: spec.method.name().into(),
            safe_url: safe_origin(&spec.url),
            duration_ms: start.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
            status,
            failure,
            attempt,
            summary: None,
        };
        let mut all = self.records.lock().await;
        let records = all.entry(job.id).or_default();
        if records.len() < 128 {
            records.push_back(record.clone());
        } else {
            let mut marker = record.clone();
            marker.summary = Some("request_history_truncated".into());
            marker.status = None;
            marker.failure = None;
            marker.attempt = 0;
            records[127] = marker;
        }
        record
    }
    async fn attempt(
        &self,
        job: &ProviderJob,
        spec: &RequestSpec,
        cancel: &CancellationToken,
    ) -> Result<(Vec<u8>, u16, Option<String>), AttemptFailure> {
        let original = Url::parse(&spec.url).map_err(|_| ProviderFailure::UnsafeTarget)?;
        if spec.secret_queries.len() + usize::from(spec.secret_query.is_some()) > 8 {
            return Err(ProviderFailure::InvalidConfiguration.into());
        }
        let mut query_names = std::collections::HashSet::new();
        for (name, secret) in spec.secret_query.iter().chain(spec.secret_queries.iter()) {
            if !query_names.insert(name)
                || name.is_empty()
                || name.len() > 64
                || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                || secret.expose().is_empty()
                || secret.expose().len() > 16384
                || secret.expose().chars().any(char::is_control)
                || original.query_pairs().any(|(key, _)| key == name.as_str())
            {
                return Err(ProviderFailure::InvalidConfiguration.into());
            }
        }
        let mut url = original.clone();
        for hop in 0..=5 {
            self.policy.validate_url(&url)?;
            super::music::validate_music_destination(&job.provider_id, &original, &url)?;
            if self.trusted_proxy {
                self.policy
                    .resolve(
                        url.host_str().ok_or(ProviderFailure::UnsafeTarget)?,
                        url.port_or_known_default().unwrap_or(443),
                    )
                    .await?;
            }
            let method = match spec.method {
                RequestMethod::Get => Method::GET,
                RequestMethod::Post => Method::POST,
            };
            let mut request = self
                .client
                .request(method, url.clone())
                .timeout(Duration::from_millis(job.config.timeout_ms));
            // Never forward authorization/cookies across origins, even on a safe redirect.
            if url.origin() == original.origin() {
                let mut headers = secret_headers(spec)?;
                if matches!(job.provider_id.as_str(), "musicbrainz" | "coverartarchive") {
                    headers.remove("User-Agent");
                }
                request = request.headers(headers);
                for (name, secret) in spec.secret_query.iter().chain(spec.secret_queries.iter()) {
                    request = request.query(&[(name.as_str(), secret.expose())]);
                }
            }
            if matches!(job.provider_id.as_str(), "musicbrainz" | "coverartarchive") {
                request = request.header(
                    "User-Agent",
                    "MediaAtlas/0.1.0 (https://github.com/susanforme/MediaAtlas)",
                );
            }
            if let Some(body) = &spec.json_body {
                request = request.json(body);
            }
            let mut response = tokio::select! {_=cancel.cancelled()=>return Err(ProviderFailure::Cancelled.into()),r=request.send()=>r.map_err(|e|classify_transport(&e))?};
            let status = response.status().as_u16();
            if response.status().is_redirection() {
                // Canonical metadata endpoints must not follow verification pages
                // or issue extra requests inside a mandatory site interval.
                if hop == 5 || spec.method != RequestMethod::Get || matches!(job.provider_id.as_str(), "musicbrainz" | "douban" | "bangumi" | "omdb")
                {
                    return Err(http_failure(ProviderFailure::UnsafeTarget, status));
                }
                let next = response
                    .headers()
                    .get(LOCATION)
                    .and_then(|h| h.to_str().ok())
                    .ok_or_else(|| http_failure(ProviderFailure::UnsafeTarget, status))?;
                url = url
                    .join(next)
                    .map_err(|_| http_failure(ProviderFailure::UnsafeTarget, status))?;
                self.policy
                    .validate_url(&url)
                    .map_err(|e| http_failure(e, status))?;
                // A remote Location may reflect the secret into a different URL.
                // Refuse crossing origins for query credentials rather than trying
                // to scrub arbitrary path/encoded copies supplied by that server.
                if (spec.secret_query.is_some() || !spec.secret_queries.is_empty())
                    && url.origin() != original.origin()
                {
                    return Err(http_failure(ProviderFailure::UnsafeTarget, status));
                }
                if spec
                    .secret_query
                    .iter()
                    .chain(spec.secret_queries.iter())
                    .any(|(name, _)| url.query_pairs().any(|(key, _)| key == name.as_str()))
                {
                    return Err(http_failure(ProviderFailure::UnsafeTarget, status));
                }
                continue;
            }
            let retry_after = response
                .headers()
                .get(RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(retry_after);
            let content_type = response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase();
            if response
                .content_length()
                .is_some_and(|n| n > job.config.max_response_bytes as u64)
            {
                return Err(http_failure(ProviderFailure::ResponseTooLarge, status));
            }
            let mut body = Vec::new();
            while let Some(chunk) = tokio::select! {_=cancel.cancelled()=>return Err(ProviderFailure::Cancelled.into()),r=response.chunk()=>r.map_err(|e| {let mut failure=classify_transport(&e);failure.status=Some(status);failure})?}
            {
                if chunk.len() > job.config.max_response_bytes.saturating_sub(body.len()) {
                    return Err(http_failure(ProviderFailure::ResponseTooLarge, status));
                }
                body.extend_from_slice(&chunk);
            }
            let captcha = looks_like_captcha(&body);
            let failure = if captcha {
                Some(ProviderFailure::Captcha)
            } else {
                match status {
                    200..=299 => None,
                    403 => Some(ProviderFailure::Blocked),
                    404 => Some(ProviderFailure::NotFound),
                    429 => Some(ProviderFailure::RateLimited),
                    400..=499 => Some(ProviderFailure::Http4xx),
                    500..=599 => Some(ProviderFailure::Http5xx),
                    _ => Some(ProviderFailure::NetworkError),
                }
            };
            if let Some(kind) = failure {
                return Err(AttemptFailure {
                    kind,
                    status: Some(status),
                    retry_after,
                });
            }
            match spec.engine {
                EngineKind::Api => {
                    if !(content_type == "application/json" || content_type.ends_with("+json")) {
                        return Err(http_failure(ProviderFailure::ParseError, status));
                    }
                    let value = parse_json(&body).map_err(|e| http_failure(e, status))?;
                    if spec.json_body.is_some()
                        && value.get("errors").is_some_and(|v| {
                            !v.is_null() && v.as_array().is_none_or(|a| !a.is_empty())
                        })
                    {
                        return Err(http_failure(ProviderFailure::ParseError, status));
                    }
                    if let Some((pointer, expected)) = &spec.expected_identity {
                        if value.pointer(pointer).and_then(Value::as_str) != Some(expected) {
                            return Err(http_failure(ProviderFailure::IdentityMismatch, status));
                        }
                    }
                }
                EngineKind::Html => {
                    if !matches!(content_type.as_str(), "text/html" | "application/xhtml+xml")
                        || std::str::from_utf8(&body).is_err()
                    {
                        return Err(http_failure(ProviderFailure::ParseError, status));
                    }
                }
                EngineKind::Image => {
                    if !valid_image(&content_type, &body) {
                        return Err(http_failure(ProviderFailure::ParseError, status));
                    }
                }
            }
            return Ok((
                body,
                status,
                (!content_type.is_empty()).then_some(content_type),
            ));
        }
        Err(ProviderFailure::UnsafeTarget.into())
    }
    /// Fetches cursor/link JSON pages without permitting cross-origin credentials.
    /// `next_pointer` is a JSON pointer to a string URL or null at the final page.
    pub async fn json_pages(
        &self,
        job: &ProviderJob,
        mut spec: RequestSpec,
        next_pointer: &str,
        max_pages: usize,
        cancel: &CancellationToken,
    ) -> Result<Vec<Value>, ProviderFailure> {
        if max_pages == 0
            || max_pages > 32
            || spec.engine != EngineKind::Api
            || spec.method != RequestMethod::Get
        {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        let origin = Url::parse(&spec.url)
            .map_err(|_| ProviderFailure::UnsafeTarget)?
            .origin();
        let mut seen = std::collections::HashSet::new();
        let mut pages = Vec::new();
        for _ in 0..max_pages {
            if !seen.insert(spec.url.clone()) {
                return Err(ProviderFailure::ParseError);
            }
            let page = self.execute(job, spec.clone(), cancel).await?.json()?;
            let next = match page.pointer(next_pointer) {
                None | Some(Value::Null) => None,
                Some(Value::String(next)) => Some(next.clone()),
                _ => return Err(ProviderFailure::ParseError),
            };
            pages.push(page);
            let Some(next) = next else {
                return Ok(pages);
            };
            let url = Url::parse(&spec.url)
                .and_then(|base| base.join(&next))
                .map_err(|_| ProviderFailure::UnsafeTarget)?;
            if url.origin() != origin {
                return Err(ProviderFailure::UnsafeTarget);
            }
            self.policy.validate_url(&url)?;
            spec.url = url.to_string();
        }
        Err(ProviderFailure::ResponseTooLarge)
    }
}
fn http_failure(kind: ProviderFailure, status: u16) -> AttemptFailure {
    AttemptFailure {
        kind,
        status: Some(status),
        retry_after: None,
    }
}
fn validate_config(c: &ProviderConfig) -> Result<(), ProviderFailure> {
    if c.timeout_ms == 0
        || c.timeout_ms > 120000
        || c.concurrency == 0
        || c.concurrency > 32
        || c.image_concurrency == 0
        || c.image_concurrency > 16
        || c.interval_ms > 60000
        || c.max_response_bytes == 0
        || c.max_response_bytes > 16 * 1024 * 1024
        || c.max_retries > 3
    {
        Err(ProviderFailure::InvalidConfiguration)
    } else {
        Ok(())
    }
}
fn secret_headers(spec: &RequestSpec) -> Result<HeaderMap, ProviderFailure> {
    let mut headers = HeaderMap::new();
    for (name, value) in [
        ("user-agent", spec.user_agent.as_deref()),
        ("referer", spec.referer.as_deref()),
    ] {
        if let Some(value) = value {
            let mut value =
                HeaderValue::from_str(value).map_err(|_| ProviderFailure::InvalidConfiguration)?;
            value.set_sensitive(true);
            headers.insert(HeaderName::from_static(name), value);
        }
    }
    for (name, value) in spec
        .bearer
        .as_ref()
        .map(|v| ("authorization".to_owned(), format!("Bearer {}", v.expose())))
        .into_iter()
        .chain(
            spec.cookie
                .as_ref()
                .map(|v| ("cookie".to_owned(), v.expose().to_owned())),
        )
        .chain(
            spec.api_key
                .as_ref()
                .map(|(k, v)| (k.clone(), v.expose().to_owned())),
        )
    {
        let name = HeaderName::from_bytes(name.as_bytes())
            .map_err(|_| ProviderFailure::InvalidConfiguration)?;
        if !matches!(name.as_str(), "authorization" | "cookie") && !name.as_str().starts_with("x-")
        {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        if name.as_str().starts_with("x-forwarded-")
            || name.as_str().starts_with("x-proxy-")
            || matches!(name.as_str(), "x-original-url" | "x-rewrite-url")
        {
            return Err(ProviderFailure::InvalidConfiguration);
        }
        let mut value =
            HeaderValue::from_str(&value).map_err(|_| ProviderFailure::InvalidConfiguration)?;
        value.set_sensitive(true);
        headers.insert(name, value);
    }
    Ok(headers)
}
fn classify_transport(error: &reqwest::Error) -> AttemptFailure {
    let kind = if error.is_timeout() {
        ProviderFailure::Timeout
    } else {
        let mut source = std::error::Error::source(error);
        let mut kind = ProviderFailure::NetworkError;
        while let Some(err) = source {
            if let Some(failure) = err.downcast_ref::<ProviderFailure>() {
                kind = *failure;
                break;
            }
            // Classification only; raw diagnostic strings never leave this function.
            let text = err.to_string().to_ascii_lowercase();
            if text.contains("certificate")
                || text.contains("tls")
                || text.contains("ssl")
                || text.contains("received corrupt")
                || text.contains("handshake")
            {
                kind = ProviderFailure::TlsError;
            } else if text.contains("dns") || text.contains("lookup") {
                kind = ProviderFailure::DnsError;
            }
            source = err.source();
        }
        kind
    };
    kind.into()
}
fn retry_after(value: &str) -> Option<Duration> {
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    httpdate::parse_http_date(value)
        .ok()
        .map(|date| date.duration_since(SystemTime::now()).unwrap_or_default())
}
fn looks_like_captcha(body: &[u8]) -> bool {
    let text = String::from_utf8_lossy(body).to_ascii_lowercase();
    [
        "g-recaptcha",
        "h-captcha",
        "cf-chl-",
        "captcha challenge",
        "name=\"captcha\"",
    ]
    .iter()
    .any(|v| text.contains(v))
}
fn valid_image(kind: &str, body: &[u8]) -> bool {
    match kind {
        "image/png" => body.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => body.starts_with(b"\xff\xd8\xff"),
        "image/webp" => body.starts_with(b"RIFF") && body.get(8..12) == Some(b"WEBP"),
        _ => false,
    }
}
