use chrono::{DateTime, Duration, Utc};
use mediaatlas_domain::{
    FileOperationRecord, FileOperationState, MediaItem, MediaKind, ScrapeRun, TaskKind, TaskRecord,
    TaskState,
};
use mediaatlas_infrastructure::{
    library::{LibraryRepository, RepositoryError},
    tasks::TaskRepository,
};
use serde_json::json;
use tempfile::tempdir;
use uuid::Uuid;

fn time() -> DateTime<Utc> {
    DateTime::from_timestamp_millis(1_700_000_000_000).unwrap()
}

fn task() -> TaskRecord {
    TaskRecord {
        id: Uuid::new_v4(),
        kind: TaskKind::SearchProviders,
        state: TaskState::Queued,
        input_snapshot: json!({"schema_version":1,"provider_id":"fixture","credential_ref":"local-vault-id"}),
        created_at: time(),
        updated_at: time(),
    }
}

#[tokio::test]
async fn state_updates_survive_reopen_without_mutating_inputs() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("tasks.sqlite");
    let library = LibraryRepository::open(&path).await.unwrap();
    let repo = TaskRepository::new(library.pool().clone());
    let input = task();
    repo.insert_task(&input).await.unwrap();
    assert_eq!(
        repo.insert_task(&input).await,
        Err(RepositoryError::Conflict)
    );
    assert!(repo
        .update_task_state(input.id, TaskState::Running, time() + Duration::seconds(1))
        .await
        .unwrap());
    assert!(!repo
        .update_task_state(Uuid::new_v4(), TaskState::Cancelled, time())
        .await
        .unwrap());
    // Input snapshots remain immutable even through direct SQL on the shared pool.
    assert!(
        sqlx::query("UPDATE scrape_tasks SET input_snapshot='{}' WHERE id=?")
            .bind(input.id.to_string())
            .execute(library.pool())
            .await
            .is_err()
    );
    library.pool().close().await;
    let reopened = LibraryRepository::open(&path).await.unwrap();
    let repo = TaskRepository::new(reopened.pool().clone());
    let saved = repo.get_task(input.id).await.unwrap().unwrap();
    assert_eq!(saved.state, TaskState::Running);
    assert_eq!(saved.input_snapshot, input.input_snapshot);
    assert_eq!(saved.created_at, input.created_at);
    assert_eq!(saved.updated_at, time() + Duration::seconds(1));
    reopened.pool().close().await;
}

#[tokio::test]
async fn audit_relationships_preserve_plans_and_never_touch_source_files() {
    let dir = tempdir().unwrap();
    let source = dir.path().join("source.mkv");
    std::fs::write(&source, b"fixture media").unwrap();
    let library = LibraryRepository::open(&dir.path().join("library.sqlite"))
        .await
        .unwrap();
    let repo = TaskRepository::new(library.pool().clone());
    let input = task();
    repo.insert_task(&input).await.unwrap();
    let media = MediaItem {
        id: Uuid::new_v4(),
        kind: MediaKind::Movie,
        title: "Fixture".into(),
        original_title: None,
        year: None,
        created_at: time(),
        updated_at: time(),
    };
    library.save_media(&media).await.unwrap();
    let run = ScrapeRun {
        id: Uuid::new_v4(),
        task_id: input.id,
        media_item_id: Some(media.id),
        provider_id: Some("fixture".into()),
        config_snapshot: json!({"language":"en","credential_ref":"vault-entry"}),
        state: TaskState::Running,
        started_at: time(),
        finished_at: None,
    };
    repo.insert_scrape_run(&run).await.unwrap();
    let operation = FileOperationRecord {
        id: Uuid::new_v4(),
        task_id: Some(input.id),
        plan: json!({"strategy":"preserve","source":source.to_str().unwrap()}),
        state: FileOperationState::Planned,
        created_at: time(),
        updated_at: time(),
    };
    repo.insert_file_operation(&operation).await.unwrap();
    repo.update_scrape_run_state(
        run.id,
        TaskState::Succeeded,
        Some(time() + Duration::seconds(1)),
    )
    .await
    .unwrap();
    repo.update_file_operation_state(
        operation.id,
        FileOperationState::Approved,
        time() + Duration::seconds(1),
    )
    .await
    .unwrap();
    assert_eq!(
        repo.get_scrape_run(run.id)
            .await
            .unwrap()
            .unwrap()
            .config_snapshot,
        run.config_snapshot
    );
    assert!(
        sqlx::query("UPDATE scrape_runs SET config_snapshot='{}' WHERE id=?")
            .bind(run.id.to_string())
            .execute(library.pool())
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE file_operations SET plan='{}' WHERE id=?")
            .bind(operation.id.to_string())
            .execute(library.pool())
            .await
            .is_err()
    );
    library.remove_media(media.id).await.unwrap();
    assert_eq!(
        repo.get_scrape_run(run.id)
            .await
            .unwrap()
            .unwrap()
            .media_item_id,
        None
    );
    sqlx::query("DELETE FROM scrape_tasks WHERE id=?")
        .bind(input.id.to_string())
        .execute(library.pool())
        .await
        .unwrap();
    assert!(repo.get_scrape_run(run.id).await.unwrap().is_none());
    let saved = repo
        .get_file_operation(operation.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.task_id, None);
    assert_eq!(saved.plan, operation.plan);
    assert_eq!(saved.state, FileOperationState::Approved);
    assert_eq!(std::fs::read(source).unwrap(), b"fixture media");
    library.pool().close().await;
}

#[tokio::test]
async fn invalid_snapshots_and_missing_parents_are_rejected_without_leaking_values() {
    let dir = tempdir().unwrap();
    let library = LibraryRepository::open(&dir.path().join("library.sqlite"))
        .await
        .unwrap();
    let repo = TaskRepository::new(library.pool().clone());
    for snapshot in [
        json!(null),
        json!({"settings":[{"api_key":"secret-fixture"}]}),
        json!({"Cookie":"secret-fixture"}),
    ] {
        let mut invalid = task();
        invalid.input_snapshot = snapshot;
        let error = repo.insert_task(&invalid).await.unwrap_err();
        assert_eq!(error, RepositoryError::InvalidInput);
        assert!(!error.to_string().contains("secret-fixture"));
        assert!(repo.get_task(invalid.id).await.unwrap().is_none());
    }
    let run = ScrapeRun {
        id: Uuid::new_v4(),
        task_id: Uuid::new_v4(),
        media_item_id: None,
        provider_id: Some("fixture".into()),
        config_snapshot: json!({}),
        state: TaskState::Running,
        started_at: time(),
        finished_at: None,
    };
    assert_eq!(
        repo.insert_scrape_run(&run).await,
        Err(RepositoryError::Conflict)
    );
    library.pool().close().await;
}
