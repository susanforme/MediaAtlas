//! Versioned provider contracts; configuration contains references, never credentials.
use crate::{MediaKind, TaskState};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFailure {
    Timeout,
    DnsError,
    TlsError,
    NetworkError,
    Http4xx,
    Http5xx,
    RateLimited,
    Blocked,
    Captcha,
    NotFound,
    ParseError,
    IdentityMismatch,
    Cancelled,
    UnsafeTarget,
    ResponseTooLarge,
    InvalidConfiguration,
    CredentialsUnavailable,
    Disabled,
    CoolingDown,
}
impl std::fmt::Display for ProviderFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Provider operation failed ({self:?})")
    }
}
impl std::error::Error for ProviderFailure {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderDescriptor {
    pub id: String,
    pub name: String,
    pub description: String,
    pub website: Option<String>,
    pub terms_url: Option<String>,
    pub privacy_url: Option<String>,
    pub license: String,
    pub media_kinds: Vec<MediaKind>,
    pub languages: Vec<String>,
    pub capabilities: Vec<String>,
    pub fixture: bool,
    pub documentation_url: Option<String>,
    pub engine_types: Vec<String>,
    pub requires_api_key: bool,
    pub requires_login: bool,
    pub rate_limit_policy: String,
    pub attribution: String,
    pub data_usage_notes: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderConfig {
    pub enabled: bool,
    pub revision: u64,
    pub timeout_ms: u64,
    pub concurrency: usize,
    pub image_concurrency: usize,
    pub interval_ms: u64,
    pub max_response_bytes: usize,
    pub max_retries: u8,
    pub credential_ref: Option<String>,
}
impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            revision: 1,
            timeout_ms: 10000,
            concurrency: 2,
            image_concurrency: 1,
            interval_ms: 250,
            max_response_bytes: 2097152,
            max_retries: 2,
            credential_ref: None,
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderJob {
    pub id: Uuid,
    pub provider_id: String,
    pub media_item_id: Option<Uuid>,
    pub media_kind: Option<MediaKind>,
    pub file_identity: Option<String>,
    pub region: Option<String>,
    pub correlation_id: Uuid,
    pub operation: String,
    pub query: String,
    pub language: String,
    pub config: ProviderConfig,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkRequestRecord {
    pub task_id: Uuid,
    pub provider_id: String,
    pub engine: String,
    pub method: String,
    pub safe_url: String,
    pub duration_ms: u64,
    pub status: Option<u16>,
    pub failure: Option<ProviderFailure>,
    pub attempt: u8,
    pub summary: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderView {
    pub descriptor: ProviderDescriptor,
    pub config: ProviderConfig,
    pub health: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderTaskView {
    pub id: Uuid,
    pub provider_id: String,
    pub state: TaskState,
    pub query: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub failure: Option<ProviderFailure>,
    pub result_summary: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderSnapshot {
    pub schema_version: u16,
    pub providers: Vec<ProviderView>,
    pub tasks: Vec<ProviderTaskView>,
    pub requests: Vec<NetworkRequestRecord>,
}

pub trait ProviderInfo {
    fn descriptor(&self) -> ProviderDescriptor;
}
pub trait SearchProvider<Q, R>: ProviderInfo {
    fn search(
        &self,
        query: Q,
        job: &ProviderJob,
    ) -> impl std::future::Future<Output = Result<Vec<R>, ProviderFailure>> + Send;
}
pub trait MetadataProvider<I, M>: ProviderInfo {
    fn metadata(
        &self,
        id: I,
        job: &ProviderJob,
    ) -> impl std::future::Future<Output = Result<M, ProviderFailure>> + Send;
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtworkCandidate {
    pub url: String,
    pub source_provider: String,
    pub kind: String,
    pub language: Option<String>,
}
pub trait ArtworkProvider<I>: ProviderInfo {
    fn artwork(
        &self,
        id: I,
        job: &ProviderJob,
    ) -> impl std::future::Future<Output = Result<Vec<ArtworkCandidate>, ProviderFailure>> + Send;
}
