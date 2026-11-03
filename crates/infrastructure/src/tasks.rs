//! Persistence of immutable intent; no execution, scheduling or file-system effects.

use chrono::{DateTime, Utc};
use mediaatlas_domain::{
    FileOperationRecord, FileOperationState, ScrapeRun, TaskRecord, TaskState,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::Value;
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};
use uuid::Uuid;

use crate::library::RepositoryError;

#[derive(Clone)]
pub struct TaskRepository {
    pool: SqlitePool,
}

impl TaskRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert_task(&self, task: &TaskRecord) -> Result<(), RepositoryError> {
        let snapshot = snapshot_json(&task.input_snapshot)?;
        sqlx::query("INSERT INTO scrape_tasks (id,kind,state,input_snapshot,created_at,updated_at) VALUES (?,?,?,?,?,?)")
            .bind(task.id.to_string()).bind(enum_text(task.kind)?).bind(enum_text(task.state)?)
            .bind(snapshot).bind(task.created_at.timestamp_millis()).bind(task.updated_at.timestamp_millis())
            .execute(&self.pool).await.map_err(database_error)?;
        Ok(())
    }

    pub async fn get_task(&self, id: Uuid) -> Result<Option<TaskRecord>, RepositoryError> {
        let row = sqlx::query("SELECT * FROM scrape_tasks WHERE id=?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(database_error)?;
        row.map(|r| {
            Ok(TaskRecord {
                id: uuid(&r, "id")?,
                kind: enumeration(&r, "kind")?,
                state: enumeration(&r, "state")?,
                input_snapshot: json(&r, "input_snapshot")?,
                created_at: timestamp(&r, "created_at")?,
                updated_at: timestamp(&r, "updated_at")?,
            })
        })
        .transpose()
    }

    pub async fn update_task_state(
        &self,
        id: Uuid,
        state: TaskState,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query("UPDATE scrape_tasks SET state=?,updated_at=? WHERE id=?")
            .bind(enum_text(state)?)
            .bind(updated_at.timestamp_millis())
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(database_error)?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn insert_scrape_run(&self, run: &ScrapeRun) -> Result<(), RepositoryError> {
        let snapshot = snapshot_json(&run.config_snapshot)?;
        sqlx::query("INSERT INTO scrape_runs (id,task_id,media_item_id,provider_id,config_snapshot,state,started_at,finished_at) VALUES (?,?,?,?,?,?,?,?)")
            .bind(run.id.to_string()).bind(run.task_id.to_string()).bind(run.media_item_id.map(|v|v.to_string()))
            .bind(&run.provider_id).bind(snapshot).bind(enum_text(run.state)?).bind(run.started_at.timestamp_millis())
            .bind(run.finished_at.map(|v|v.timestamp_millis())).execute(&self.pool).await.map_err(database_error)?;
        Ok(())
    }

    pub async fn get_scrape_run(&self, id: Uuid) -> Result<Option<ScrapeRun>, RepositoryError> {
        let row = sqlx::query("SELECT * FROM scrape_runs WHERE id=?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(database_error)?;
        row.map(|r| {
            Ok(ScrapeRun {
                id: uuid(&r, "id")?,
                task_id: uuid(&r, "task_id")?,
                media_item_id: optional_uuid(&r, "media_item_id")?,
                provider_id: r
                    .try_get("provider_id")
                    .map_err(|_| RepositoryError::CorruptData)?,
                config_snapshot: json(&r, "config_snapshot")?,
                state: enumeration(&r, "state")?,
                started_at: timestamp(&r, "started_at")?,
                finished_at: optional_timestamp(&r, "finished_at")?,
            })
        })
        .transpose()
    }

    pub async fn update_scrape_run_state(
        &self,
        id: Uuid,
        state: TaskState,
        finished_at: Option<DateTime<Utc>>,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query("UPDATE scrape_runs SET state=?,finished_at=? WHERE id=?")
            .bind(enum_text(state)?)
            .bind(finished_at.map(|v| v.timestamp_millis()))
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(database_error)?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn insert_file_operation(
        &self,
        operation: &FileOperationRecord,
    ) -> Result<(), RepositoryError> {
        let plan = snapshot_json(&operation.plan)?;
        sqlx::query("INSERT INTO file_operations (id,task_id,plan,state,created_at,updated_at) VALUES (?,?,?,?,?,?)")
            .bind(operation.id.to_string()).bind(operation.task_id.map(|v|v.to_string()))
            .bind(plan).bind(enum_text(operation.state)?).bind(operation.created_at.timestamp_millis())
            .bind(operation.updated_at.timestamp_millis()).execute(&self.pool).await.map_err(database_error)?;
        Ok(())
    }

    pub async fn get_file_operation(
        &self,
        id: Uuid,
    ) -> Result<Option<FileOperationRecord>, RepositoryError> {
        let row = sqlx::query("SELECT * FROM file_operations WHERE id=?")
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(database_error)?;
        row.map(|r| {
            Ok(FileOperationRecord {
                id: uuid(&r, "id")?,
                task_id: optional_uuid(&r, "task_id")?,
                plan: json(&r, "plan")?,
                state: enumeration(&r, "state")?,
                created_at: timestamp(&r, "created_at")?,
                updated_at: timestamp(&r, "updated_at")?,
            })
        })
        .transpose()
    }

    pub async fn update_file_operation_state(
        &self,
        id: Uuid,
        state: FileOperationState,
        updated_at: DateTime<Utc>,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query("UPDATE file_operations SET state=?,updated_at=? WHERE id=?")
            .bind(enum_text(state)?)
            .bind(updated_at.timestamp_millis())
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(database_error)?;
        Ok(result.rows_affected() == 1)
    }
}

fn database_error(error: sqlx::Error) -> RepositoryError {
    if error.as_database_error().is_some_and(|e| {
        e.is_unique_violation() || e.is_foreign_key_violation() || e.is_check_violation()
    }) {
        RepositoryError::Conflict
    } else {
        RepositoryError::Database
    }
}

fn enum_text(value: impl Serialize) -> Result<String, RepositoryError> {
    match serde_json::to_value(value).map_err(|_| RepositoryError::InvalidInput)? {
        Value::String(value) => Ok(value),
        _ => Err(RepositoryError::InvalidInput),
    }
}

fn enumeration<T: DeserializeOwned>(row: &SqliteRow, name: &str) -> Result<T, RepositoryError> {
    let text: String = row
        .try_get(name)
        .map_err(|_| RepositoryError::CorruptData)?;
    serde_json::from_value(Value::String(text)).map_err(|_| RepositoryError::CorruptData)
}

fn uuid(row: &SqliteRow, name: &str) -> Result<Uuid, RepositoryError> {
    let text: String = row
        .try_get(name)
        .map_err(|_| RepositoryError::CorruptData)?;
    Uuid::parse_str(&text).map_err(|_| RepositoryError::CorruptData)
}

fn optional_uuid(row: &SqliteRow, name: &str) -> Result<Option<Uuid>, RepositoryError> {
    let text: Option<String> = row
        .try_get(name)
        .map_err(|_| RepositoryError::CorruptData)?;
    text.map(|v| Uuid::parse_str(&v).map_err(|_| RepositoryError::CorruptData))
        .transpose()
}

fn timestamp(row: &SqliteRow, name: &str) -> Result<DateTime<Utc>, RepositoryError> {
    let millis: i64 = row
        .try_get(name)
        .map_err(|_| RepositoryError::CorruptData)?;
    DateTime::from_timestamp_millis(millis).ok_or(RepositoryError::CorruptData)
}

fn optional_timestamp(
    row: &SqliteRow,
    name: &str,
) -> Result<Option<DateTime<Utc>>, RepositoryError> {
    let millis: Option<i64> = row
        .try_get(name)
        .map_err(|_| RepositoryError::CorruptData)?;
    millis
        .map(|v| DateTime::from_timestamp_millis(v).ok_or(RepositoryError::CorruptData))
        .transpose()
}

fn json(row: &SqliteRow, name: &str) -> Result<Value, RepositoryError> {
    let text: String = row
        .try_get(name)
        .map_err(|_| RepositoryError::CorruptData)?;
    serde_json::from_str(&text).map_err(|_| RepositoryError::CorruptData)
}

// Reject common credential keys recursively. Callers must still provide sanitized
// snapshots and credential references; this cannot detect secrets in arbitrary text.
fn snapshot_json(value: &Value) -> Result<String, RepositoryError> {
    fn contains_credential(value: &Value) -> bool {
        match value {
            Value::Object(fields) => fields.iter().any(|(key, value)| {
                let key = key.to_ascii_lowercase().replace(['_', '-'], "");
                matches!(
                    key.as_str(),
                    "apikey"
                        | "token"
                        | "accesstoken"
                        | "refreshtoken"
                        | "authorization"
                        | "cookie"
                        | "setcookie"
                        | "password"
                        | "secret"
                ) || contains_credential(value)
            }),
            Value::Array(items) => items.iter().any(contains_credential),
            _ => false,
        }
    }
    if !value.is_object() || contains_credential(value) {
        return Err(RepositoryError::InvalidInput);
    }
    serde_json::to_string(value).map_err(|_| RepositoryError::InvalidInput)
}
