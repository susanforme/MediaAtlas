use mediaatlas_domain::{
    ProviderConfig, ProviderDescriptor, ProviderFailure, ProviderInfo, ProviderJob, TaskState,
};
use mediaatlas_infrastructure::{
    library::{LibraryRepository, RepositoryError},
    network::{CancellationToken, NetworkEngine},
    providers::{OfflineFixtureProvider, ProviderAdapter, ProviderService, FIXTURE_PROVIDER_ID},
};
use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
use tempfile::TempDir;
use tokio::sync::{mpsc, Semaphore};
use uuid::Uuid;

async fn setup() -> (TempDir, LibraryRepository, ProviderService) {
    let dir = tempfile::tempdir().unwrap();
    let repo = LibraryRepository::open(&dir.path().join("providers.sqlite"))
        .await
        .unwrap();
    let service = ProviderService::new(repo.clone()).await.unwrap();
    (dir, repo, service)
}
async fn terminal(service: &ProviderService, id: Uuid) -> TaskState {
    for _ in 0..200 {
        let task = service
            .snapshot()
            .await
            .unwrap()
            .tasks
            .into_iter()
            .find(|t| t.id == id)
            .unwrap();
        if !matches!(task.state, TaskState::Queued | TaskState::Running) {
            return task.state;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("fixture task did not complete");
}

#[tokio::test]
async fn registration_has_no_implicit_work_and_default_disabled() {
    let (_dir, repo, service) = setup().await;
    let snapshot = service.snapshot().await.unwrap();
    assert_eq!(snapshot.providers.len(), 1);
    assert!(snapshot.providers[0].descriptor.fixture);
    assert_eq!(snapshot.providers[0].health, "disabled");
    assert!(snapshot.tasks.is_empty());
    assert!(snapshot.requests.is_empty());
    assert_eq!(
        service.start(FIXTURE_PROVIDER_ID, "Movie").await,
        Err(RepositoryError::InvalidInput)
    );
    assert_eq!(
        service.start("unknown", "Movie").await,
        Err(RepositoryError::NotFound)
    );
    let tasks: i64 = sqlx::query_scalar("SELECT count(*) FROM scrape_tasks")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(tasks, 0);
}

#[tokio::test]
async fn explicit_fixture_combines_parsers_persists_history_and_shared_task_audit() {
    let (dir, repo, service) = setup().await;
    service
        .set_enabled(FIXTURE_PROVIDER_ID, true)
        .await
        .unwrap();
    let id = service
        .start(FIXTURE_PROVIDER_ID, "Fixture movie")
        .await
        .unwrap();
    assert_eq!(terminal(&service, id).await, TaskState::Succeeded);
    let snapshot = service.snapshot().await.unwrap();
    assert_eq!(
        snapshot.tasks[0].result_summary.as_deref(),
        Some("fixture_combined")
    );
    assert_eq!(snapshot.requests.len(), 2);
    for record in &snapshot.requests {
        assert_eq!(record.method, "FIXTURE");
        assert_eq!(record.safe_url, "fixture://offline");
        assert_eq!(record.status, None);
        assert_eq!(record.task_id, id);
    }
    let run_state: String = sqlx::query_scalar("SELECT state FROM scrape_runs WHERE task_id=?")
        .bind(id.to_string())
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(run_state, "succeeded");
    drop(service);
    repo.pool().close().await;
    let reopened = LibraryRepository::open(&dir.path().join("providers.sqlite"))
        .await
        .unwrap();
    let service = ProviderService::new(reopened).await.unwrap();
    assert_eq!(service.snapshot().await.unwrap().tasks, snapshot.tasks);
    assert_eq!(
        service.snapshot().await.unwrap().requests,
        snapshot.requests
    );
    assert!(
        service.snapshot().await.unwrap().providers[0]
            .config
            .enabled
    );
}

const CANCELLATION_FIXTURE_ID: &str = "fixture.controlled-cancellation";

/// Hold parser completion until the test explicitly permits success. Cancellation
/// is a separate event, so a slow database cannot let the fixture finish first.
struct CancellationFixtureProvider {
    started: mpsc::UnboundedSender<Uuid>,
    exited: mpsc::UnboundedSender<(Uuid, Option<ProviderFailure>)>,
    complete: Arc<Semaphore>,
}

impl ProviderAdapter for CancellationFixtureProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        let mut descriptor = ProviderInfo::descriptor(&OfflineFixtureProvider);
        descriptor.id = CANCELLATION_FIXTURE_ID.into();
        descriptor
    }

    fn execute<'a>(
        &'a self,
        job: &'a ProviderJob,
        _engine: &'a NetworkEngine,
        cancel: &'a CancellationToken,
    ) -> Pin<Box<dyn Future<Output = Result<String, ProviderFailure>> + Send + 'a>> {
        Box::pin(async move {
            self.started.send(job.id).unwrap();
            let result = tokio::select! {
                _ = cancel.cancelled() => Err(ProviderFailure::Cancelled),
                permit = self.complete.acquire() => {
                    permit.unwrap().forget();
                    Ok("fixture_combined".into())
                }
            };
            self.exited
                .send((job.id, result.as_ref().err().copied()))
                .unwrap();
            result
        })
    }
}

async fn event<T>(receiver: &mut mpsc::UnboundedReceiver<T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), receiver.recv())
        .await
        .expect("controlled fixture did not reach the expected stage")
        .expect("controlled fixture stopped before the expected event")
}

#[tokio::test]
async fn cancel_and_disable_are_terminal_and_do_not_overwrite_success() {
    let (_dir, _repo, service) = setup().await;
    let (started, mut starts) = mpsc::unbounded_channel();
    let (exited, mut exits) = mpsc::unbounded_channel();
    let complete = Arc::new(Semaphore::new(0));
    service
        .register_adapter(Arc::new(CancellationFixtureProvider {
            started,
            exited,
            complete: complete.clone(),
        }))
        .await
        .unwrap();
    service
        .set_enabled(CANCELLATION_FIXTURE_ID, true)
        .await
        .unwrap();
    let id = service
        .start(CANCELLATION_FIXTURE_ID, "First")
        .await
        .unwrap();
    assert_eq!(event(&mut starts).await, id);
    let cancelled = service
        .start(CANCELLATION_FIXTURE_ID, "Second")
        .await
        .unwrap();
    assert_eq!(event(&mut starts).await, cancelled);
    assert!(service.cancel(cancelled).await.unwrap());
    assert_eq!(
        event(&mut exits).await,
        (cancelled, Some(ProviderFailure::Cancelled))
    );
    assert!(!service.cancel(cancelled).await.unwrap());
    assert!(!service.cancel(Uuid::new_v4()).await.unwrap());
    service
        .set_enabled(CANCELLATION_FIXTURE_ID, false)
        .await
        .unwrap();
    assert_eq!(
        event(&mut exits).await,
        (id, Some(ProviderFailure::Cancelled))
    );
    assert_eq!(terminal(&service, id).await, TaskState::Cancelled);
    assert_eq!(terminal(&service, cancelled).await, TaskState::Cancelled);
    let snapshot = service.snapshot().await.unwrap();
    assert!(snapshot
        .tasks
        .iter()
        .all(|t| t.failure == Some(ProviderFailure::Cancelled)));
    assert!(snapshot.requests.is_empty());

    service
        .set_enabled(CANCELLATION_FIXTURE_ID, true)
        .await
        .unwrap();
    let done = service
        .start(CANCELLATION_FIXTURE_ID, "Third")
        .await
        .unwrap();
    assert_eq!(event(&mut starts).await, done);
    complete.add_permits(1);
    assert_eq!(event(&mut exits).await, (done, None));
    assert_eq!(terminal(&service, done).await, TaskState::Succeeded);
    assert!(!service.cancel(done).await.unwrap());
    service
        .set_enabled(CANCELLATION_FIXTURE_ID, false)
        .await
        .unwrap();
    let snapshot = service.snapshot().await.unwrap();
    let completed = snapshot.tasks.iter().find(|task| task.id == done).unwrap();
    assert_eq!(completed.state, TaskState::Succeeded);
    assert_eq!(completed.failure, None);
    assert_eq!(
        completed.result_summary.as_deref(),
        Some("fixture_combined")
    );
    assert_eq!(snapshot.requests.len(), 2);
    assert!(snapshot
        .requests
        .iter()
        .all(|request| request.task_id == done));
}

#[tokio::test]
async fn config_snapshots_are_immutable_and_new_revision_only_affects_new_jobs() {
    let (_dir, repo, service) = setup().await;
    service
        .set_enabled(FIXTURE_PROVIDER_ID, true)
        .await
        .unwrap();
    let old = service.snapshot().await.unwrap().providers[0]
        .config
        .clone();
    let id = service
        .start(FIXTURE_PROVIDER_ID, "Old config")
        .await
        .unwrap();
    let mut new = old.clone();
    new.interval_ms = 1;
    new.timeout_ms = 2000;
    service
        .update_config(FIXTURE_PROVIDER_ID, &new)
        .await
        .unwrap();
    let id2 = service
        .start(FIXTURE_PROVIDER_ID, "New config")
        .await
        .unwrap();
    let before: String =
        sqlx::query_scalar("SELECT job_json FROM provider_sessions WHERE task_id=?")
            .bind(id.to_string())
            .fetch_one(repo.pool())
            .await
            .unwrap();
    let after: String =
        sqlx::query_scalar("SELECT job_json FROM provider_sessions WHERE task_id=?")
            .bind(id2.to_string())
            .fetch_one(repo.pool())
            .await
            .unwrap();
    let before: mediaatlas_domain::ProviderJob = serde_json::from_str(&before).unwrap();
    let after: mediaatlas_domain::ProviderJob = serde_json::from_str(&after).unwrap();
    assert_eq!(before.config, old);
    assert_eq!(after.config.interval_ms, 1);
    assert_eq!(after.config.revision, old.revision + 1);
    assert_eq!(before.query, "Old config");
    assert_eq!(after.query, "New config");
    assert!(
        sqlx::query("UPDATE provider_sessions SET job_json='{}' WHERE task_id=?")
            .bind(id.to_string())
            .execute(repo.pool())
            .await
            .is_err()
    );
    assert_eq!(terminal(&service, id).await, TaskState::Succeeded);
    assert_eq!(terminal(&service, id2).await, TaskState::Succeeded);
}

#[tokio::test]
async fn recovery_scopes_to_provider_tasks_without_replaying_or_altering_scan_tasks() {
    let (_dir, repo, service) = setup().await;
    service
        .set_enabled(FIXTURE_PROVIDER_ID, true)
        .await
        .unwrap();
    let id = service.start(FIXTURE_PROVIDER_ID, "Recover").await.unwrap();
    terminal(&service, id).await;
    sqlx::query("UPDATE scrape_tasks SET state='running' WHERE id=?")
        .bind(id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE scrape_runs SET state='running',finished_at=NULL WHERE task_id=?")
        .bind(id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    let scan = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES (?,'scan_library','running','{}',1,1)").bind(&scan).execute(repo.pool()).await.unwrap();
    let reopened = ProviderService::new(repo.clone()).await.unwrap();
    assert_eq!(
        reopened.snapshot().await.unwrap().tasks[0].state,
        TaskState::Interrupted
    );
    assert_eq!(reopened.recover_interrupted().await.unwrap(), 0);
    let state: String = sqlx::query_scalar("SELECT state FROM scrape_tasks WHERE id=?")
        .bind(scan)
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(state, "running");
    let state: String = sqlx::query_scalar("SELECT state FROM scrape_runs WHERE task_id=?")
        .bind(id.to_string())
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(state, "interrupted");
}

#[tokio::test]
async fn input_validation_rejects_url_secrets_and_invalid_config_without_persisting() {
    let (_dir, repo, service) = setup().await;
    service
        .set_enabled(FIXTURE_PROVIDER_ID, true)
        .await
        .unwrap();
    for query in [
        "",
        "https://example.com/path?token=secret",
        "Movie\nCookie: secret",
        "token=secret",
        "API_KEY=secret",
    ] {
        assert_eq!(
            service.start(FIXTURE_PROVIDER_ID, query).await,
            Err(RepositoryError::InvalidInput)
        );
    }
    assert_eq!(
        service.start(FIXTURE_PROVIDER_ID, &"a".repeat(257)).await,
        Err(RepositoryError::InvalidInput)
    );
    let mut invalid = ProviderConfig {
        enabled: true,
        concurrency: 0,
        ..Default::default()
    };
    assert_eq!(
        service.update_config(FIXTURE_PROVIDER_ID, &invalid).await,
        Err(RepositoryError::InvalidInput)
    );
    invalid.concurrency = 1;
    invalid.credential_ref = Some("secret".into());
    assert_eq!(
        service.update_config(FIXTURE_PROVIDER_ID, &invalid).await,
        Err(RepositoryError::InvalidInput)
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM provider_sessions")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(rows, 0);
}

#[tokio::test]
async fn language_and_context_are_immutable_and_unconnected_media_context_is_rejected() {
    use mediaatlas_domain::{MediaKind, ProviderJob};
    use mediaatlas_infrastructure::providers::ProviderJobContext;
    let (_dir, repo, service) = setup().await;
    service
        .set_enabled(FIXTURE_PROVIDER_ID, true)
        .await
        .unwrap();
    let correlation = Uuid::new_v4();
    let context = ProviderJobContext {
        region: Some("CN".into()),
        correlation_id: Some(correlation),
        ..Default::default()
    };
    let first = service
        .start_context(FIXTURE_PROVIDER_ID, "中文检索", "zh-CN", &context)
        .await
        .unwrap();
    let second = service
        .start_localized(FIXTURE_PROVIDER_ID, "English query", "en-US")
        .await
        .unwrap();
    let first_job: String =
        sqlx::query_scalar("SELECT job_json FROM provider_sessions WHERE task_id=?")
            .bind(first.to_string())
            .fetch_one(repo.pool())
            .await
            .unwrap();
    let second_job: String =
        sqlx::query_scalar("SELECT job_json FROM provider_sessions WHERE task_id=?")
            .bind(second.to_string())
            .fetch_one(repo.pool())
            .await
            .unwrap();
    let first_job: ProviderJob = serde_json::from_str(&first_job).unwrap();
    let second_job: ProviderJob = serde_json::from_str(&second_job).unwrap();
    assert_eq!(first_job.language, "zh-CN");
    assert_eq!(second_job.language, "en-US");
    assert_eq!(first_job.correlation_id, correlation);
    assert_eq!(first_job.region.as_deref(), Some("CN"));
    assert_eq!(first_job.media_kind, Some(MediaKind::Movie));
    assert_eq!(first_job.media_item_id, None);
    assert_eq!(first_job.file_identity, None);
    for lang in ["en", "zh", "fr-FR", ""] {
        assert_eq!(
            service
                .start_localized(FIXTURE_PROVIDER_ID, "Movie", lang)
                .await,
            Err(RepositoryError::InvalidInput)
        );
    }
    for invalid in [
        ProviderJobContext {
            media_kind: MediaKind::AdultVideo,
            ..Default::default()
        },
        ProviderJobContext {
            media_item_id: Some(Uuid::new_v4()),
            ..Default::default()
        },
        ProviderJobContext {
            file_identity: Some("media.mkv".into()),
            ..Default::default()
        },
        ProviderJobContext {
            region: Some("secret".into()),
            ..Default::default()
        },
    ] {
        assert_eq!(
            service
                .start_context(FIXTURE_PROVIDER_ID, "Movie", "zh-CN", &invalid)
                .await,
            Err(RepositoryError::InvalidInput)
        );
    }
    assert_eq!(terminal(&service, first).await, TaskState::Succeeded);
    assert_eq!(terminal(&service, second).await, TaskState::Succeeded);
}
