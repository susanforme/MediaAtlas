//! Persisted task intent only. Scheduling and file execution belong to later phases.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    ScanLibrary,
    ParseFile,
    SearchProviders,
    FetchMetadata,
    DownloadArtwork,
    MergeMetadata,
    ExportNfo,
    WriteAudioTags,
    OrganizeFiles,
    RefreshLibrary,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    Queued,
    Running,
    WaitingForUser,
    Succeeded,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: Uuid,
    pub kind: TaskKind,
    pub state: TaskState,
    pub input_snapshot: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScrapeRun {
    pub id: Uuid,
    pub task_id: Uuid,
    pub media_item_id: Option<Uuid>,
    pub provider_id: Option<String>,
    pub config_snapshot: Value,
    pub state: TaskState,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileOperationState {
    Planned,
    Approved,
    Running,
    Succeeded,
    Failed,
    RolledBack,
    Interrupted,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileOperationRecord {
    pub id: Uuid,
    pub task_id: Option<Uuid>,
    pub plan: Value,
    pub state: FileOperationState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn persisted_intent_uses_stable_state_ids_and_utc() {
        let time = DateTime::from_timestamp_millis(1_700_000_000_000).unwrap();
        let task = TaskRecord {
            id: Uuid::new_v4(),
            kind: TaskKind::OrganizeFiles,
            state: TaskState::WaitingForUser,
            input_snapshot: json!({"strategy":"preserve","schema_version":1}),
            created_at: time,
            updated_at: time,
        };
        let value = serde_json::to_value(&task).unwrap();
        assert_eq!(value["kind"], "organize_files");
        assert_eq!(value["state"], "waiting_for_user");
        assert!(value["created_at"].as_str().unwrap().ends_with('Z'));
        assert_eq!(serde_json::from_value::<TaskRecord>(value).unwrap(), task);
    }
}
