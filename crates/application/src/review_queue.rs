//! Durable manual review bookkeeping; never an authorization to replay writes.
use chrono::{DateTime, Utc};
use mediaatlas_domain::MediaKind;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewContext {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub root: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<MediaKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub q: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page: Option<u32>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewQueueCreate {
    pub media_ids: Vec<Uuid>,
    pub context: ReviewContext,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewQueueId {
    pub queue_id: Uuid,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewQueueList {}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewState {
    Pending,
    Deferred,
    Confirmed,
    Failed,
}
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewAction {
    Select,
    Defer,
    Retry,
    Failed,
    Confirmed,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewQueueUpdate {
    pub queue_id: Uuid,
    pub media_id: Uuid,
    pub expected_revision: u64,
    pub action: ReviewAction,
    pub checkpoint: Option<Uuid>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewQueueCheckpoint {
    pub queue_id: Uuid,
    pub media_id: Uuid,
    pub expected_revision: u64,
    pub preview_id: Uuid,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewQueueItem {
    pub media_id: Uuid,
    pub position: u32,
    pub state: ReviewState,
    pub checkpoint: Option<Uuid>,
    pub preview_id: Option<Uuid>,
    pub needs_recheck: bool,
    pub reason: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewQueueView {
    pub id: Uuid,
    pub revision: u64,
    pub current_media_id: Option<Uuid>,
    pub context: ReviewContext,
    pub items: Vec<ReviewQueueItem>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
