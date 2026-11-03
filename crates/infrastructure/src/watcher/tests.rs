use super::*;
use crate::browse::BrowseService;
use mediaatlas_application::library::{ConfirmInput, PreviewInput, RootInput};
use mediaatlas_domain::TaskState;
use tempfile::TempDir;

async fn fixture() -> (
    TempDir,
    LibraryRepository,
    ScanService,
    WatcherService,
    LibraryRoot,
) {
    let dir = crate::test_support::tempdir().unwrap();
    let path = dir.path().join("media");
    std::fs::create_dir(&path).unwrap();
    let repo = LibraryRepository::open(&dir.path().join("db.sqlite"))
        .await
        .unwrap();
    let root = BrowseService::new(repo.clone(), dir.path().join("cache"))
        .save_root(&RootInput {
            id: None,
            name: "Fixture".into(),
            path: path.to_str().unwrap().into(),
            kind: Some(MediaKind::Movie),
            enabled: true,
            exclude_patterns: vec![],
        })
        .await
        .unwrap()
        .root;
    let scans = ScanService::new(repo.clone());
    let watcher = WatcherService::new(repo.clone(), scans.clone());
    (dir, repo, scans, watcher, root)
}
async fn task_count(repo: &LibraryRepository) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM scan_sessions")
        .fetch_one(repo.pool())
        .await
        .unwrap()
}
async fn wait_tasks(repo: &LibraryRepository, count: i64) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if task_count(repo).await >= count {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("native watcher did not schedule scan");
}
async fn settled(scans: &ScanService, id: Uuid) {
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
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
}
#[tokio::test]
async fn native_events_are_debounced_into_unconfirmed_candidates_and_restart_restores_opt_in() {
    let (_dir, repo, scans, watcher, root) = fixture().await;
    assert!(!watcher.list().await.unwrap()[0].opted_in);
    assert!(watcher.configure(root.id, true).await.unwrap().active);
    let path = Path::new(&root.path).join("Fixture.2020.mkv");
    for _ in 0..12 {
        std::fs::write(&path, b"generated fixture").unwrap();
    }
    wait_tasks(&repo, 1).await;
    let task = scans.list_tasks(false).await.unwrap().remove(0);
    settled(&scans, task.id).await;
    tokio::time::sleep(Duration::from_millis(700)).await;
    assert_eq!(task_count(&repo).await, 1);
    let imported: i64 = sqlx::query_scalar("SELECT count(*) FROM media_items")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(imported, 0);
    let preview = scans
        .preview(&PreviewInput {
            task_id: task.id,
            cursor: None,
            limit: 100,
            include_adult: false,
        })
        .await
        .unwrap();
    assert_eq!(preview.items.len(), 1);
    assert_eq!(std::fs::read(&path).unwrap(), b"generated fixture");
    watcher.shutdown().await;
    assert!(!watcher.list().await.unwrap()[0].active);
    let restarted = WatcherService::new(repo.clone(), scans.clone());
    assert!(!restarted.list().await.unwrap()[0].active);
    restarted.restore().await.unwrap();
    assert!(restarted.list().await.unwrap()[0].active);
    // Changing media while the preview waits cannot create duplicate tasks.
    std::fs::write(
        Path::new(&root.path).join("Second.2021.mkv"),
        b"second fixture",
    )
    .unwrap();
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(task_count(&repo).await, 1);
    scans
        .confirm(&ConfirmInput {
            task_id: task.id,
            candidate_ids: preview.items.iter().map(|i| i.id).collect(),
        })
        .await
        .unwrap();
    wait_tasks(&repo, 2).await;
    assert_eq!(task_count(&repo).await, 2);
    restarted.shutdown().await;
}
#[tokio::test]
async fn overflow_active_scan_merge_and_root_lifecycle_preserve_one_pipeline() {
    let (_dir, repo, scans, watcher, mut root) = fixture().await;
    std::fs::write(Path::new(&root.path).join("Fixture.2020.mkv"), b"generated").unwrap();
    let manual = scans.start(root.id).await.unwrap();
    settled(&scans, manual.id).await;
    watcher.configure(root.id, true).await.unwrap();
    let signal = watcher.runtime.live.lock().unwrap().handles[&root.id].signal;
    record_overflow(&watcher.runtime.overflow, signal);
    tokio::time::sleep(Duration::from_millis(650)).await;
    assert_eq!(task_count(&repo).await, 1);
    assert!(watcher.list().await.unwrap()[0].pending_rescan);
    scans.cancel(manual.id).await.unwrap();
    wait_tasks(&repo, 2).await;
    root.enabled = false;
    root.updated_at = Utc::now();
    repo.save_root(&root).await.unwrap();
    watcher.reconcile().await.unwrap();
    assert!(!watcher.list().await.unwrap()[0].active);
    root.enabled = true;
    root.updated_at = Utc::now();
    repo.save_root(&root).await.unwrap();
    watcher.reconcile().await.unwrap();
    assert!(watcher.list().await.unwrap()[0].active);
    root.kind = Some(MediaKind::AdultVideo);
    root.updated_at = Utc::now();
    repo.save_root(&root).await.unwrap();
    watcher.reconcile().await.unwrap();
    assert!(watcher.list().await.unwrap().is_empty());
    assert!(watcher.runtime.live.lock().unwrap().handles.is_empty());
    assert_eq!(
        watcher.configure(root.id, true).await.unwrap_err(),
        RepositoryError::InvalidInput
    );
    repo.remove_root(root.id).await.unwrap();
    watcher.reconcile().await.unwrap();
    assert!(watcher.runtime.live.lock().unwrap().handles.is_empty());
    watcher.shutdown().await;
}
#[tokio::test]
async fn private_quarantine_events_are_ignored_and_user_hidden_files_are_preserved() {
    let (_dir, repo, scans, watcher, root) = fixture().await;
    watcher.configure(root.id, true).await.unwrap();
    let hidden = Path::new(&root.path).join(".mediaatlas-tag-backup");
    std::fs::create_dir(&hidden).unwrap();
    std::fs::write(hidden.join("Original.2020.mkv"), b"quarantine").unwrap();
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(task_count(&repo).await, 0);
    let user_hidden = Path::new(&root.path).join(".user");
    std::fs::create_dir(&user_hidden).unwrap();
    std::fs::write(user_hidden.join("Visible.2021.mkv"), b"user hidden").unwrap();
    wait_tasks(&repo, 1).await;
    let task = scans.list_tasks(false).await.unwrap().remove(0);
    settled(&scans, task.id).await;
    let preview = scans
        .preview(&PreviewInput {
            task_id: task.id,
            cursor: None,
            limit: 100,
            include_adult: false,
        })
        .await
        .unwrap();
    assert_eq!(preview.items.len(), 1);
    assert!(preview.items[0].relative_path.starts_with(".user/"));
    watcher.shutdown().await;
}
#[test]
fn access_events_and_only_private_paths_do_not_enqueue() {
    let event = Event::new(EventKind::Access(notify::event::AccessKind::Any));
    assert!(!event_relevant(&event));
    let event = Event::new(EventKind::Any).add_path("root/.mediaatlas_tag/source.mkv".into());
    assert!(!event_relevant(&event));
    let event = event.add_path("root/.user/movie.mkv".into());
    assert!(event_relevant(&event));
}

#[tokio::test]
async fn queued_old_generation_and_overflow_cannot_trigger_reenabled_or_reconfigured_root() {
    let (dir, repo, scans, watcher, mut root) = fixture().await;
    watcher.configure(root.id, true).await.unwrap();
    let old = watcher.runtime.live.lock().unwrap().handles[&root.id].signal;
    watcher.configure(root.id, false).await.unwrap();
    watcher.configure(root.id, true).await.unwrap();
    let current = watcher.runtime.live.lock().unwrap().handles[&root.id].signal;
    assert_ne!(old, current);
    watcher.runtime.sender.send(old).await.unwrap();
    record_overflow(&watcher.runtime.overflow, old);
    tokio::time::sleep(Duration::from_millis(650)).await;
    assert_eq!(task_count(&repo).await, 0);
    let previous = root.clone();
    let next = dir.path().join("new-media");
    std::fs::create_dir(&next).unwrap();
    root.path = next.to_str().unwrap().into();
    root.updated_at = Utc::now();
    repo.save_root(&root).await.unwrap();
    watcher.reconcile().await.unwrap();
    watcher.runtime.sender.send(current).await.unwrap();
    record_overflow(&watcher.runtime.overflow, current);
    tokio::time::sleep(Duration::from_millis(650)).await;
    assert_eq!(task_count(&repo).await, 0);
    assert_eq!(
        scans.start_watched(&previous).await.unwrap_err(),
        RepositoryError::Conflict
    );
    watcher.configure(root.id, false).await.unwrap();
    assert_eq!(
        scans.start_watched(&root).await.unwrap_err(),
        RepositoryError::Conflict
    );
    watcher.shutdown().await;
}

#[test]
fn scanner_prunes_private_staging_without_implicit_git_or_hidden_exclusions() {
    use mediaatlas_media_scanner::{run_scan, ScanOptions};
    let dir = crate::test_support::tempdir().unwrap();
    for relative in [
        ".mediaatlas-tag-backup/Original.2020.mkv",
        ".mediaatlas_org/source.mkv",
        ".user/Visible.2021.mkv",
        "ordinary/Visible.2022.mkv",
        "excluded/Hidden.2023.mkv",
    ] {
        let path = dir.path().join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"fixture").unwrap();
    }
    std::fs::write(dir.path().join(".gitignore"), b"ordinary/\n.user/\n").unwrap();
    let options = ScanOptions {
        kind: Some(MediaKind::Movie),
        path_style: crate::test_support::native_path_style(),
        exclude_patterns: vec!["excluded/**".into()],
        known_files: HashMap::new(),
    };
    let mut found = Vec::new();
    run_scan(
        dir.path(),
        &options,
        &AtomicBool::new(false),
        |entry| {
            found.push(entry.relative_path);
            true
        },
        |_| {},
    )
    .unwrap();
    found.sort();
    assert_eq!(
        found,
        vec![".user/Visible.2021.mkv", "ordinary/Visible.2022.mkv"]
    );
}

#[test]
fn bounded_queue_overflow_retains_full_rescan_signal_with_bounded_memory() {
    let (sender, mut receiver) = mpsc::channel(QUEUE_CAPACITY);
    let overflow = Mutex::new(HashSet::new());
    let signal = Signal {
        root_id: Uuid::new_v4(),
        generation: Uuid::new_v4(),
    };
    for _ in 0..QUEUE_CAPACITY {
        enqueue(&sender, &overflow, signal);
    }
    assert!(overflow.lock().unwrap().is_empty());
    enqueue(&sender, &overflow, signal);
    assert!(overflow.lock().unwrap().contains(&signal));
    for _ in 0..QUEUE_CAPACITY {
        assert_eq!(receiver.try_recv().unwrap(), signal);
    }
    for _ in 0..QUEUE_CAPACITY + 20 {
        record_overflow(
            &overflow,
            Signal {
                root_id: Uuid::new_v4(),
                generation: Uuid::new_v4(),
            },
        );
    }
    assert_eq!(overflow.lock().unwrap().len(), QUEUE_CAPACITY);
}

#[tokio::test]
async fn shutdown_acknowledgement_waits_for_dispatch_and_rejects_queued_events() {
    let (_dir, repo, _scans, watcher, root) = fixture().await;
    watcher.configure(root.id, true).await.unwrap();
    let signal = watcher.runtime.live.lock().unwrap().handles[&root.id].signal;
    let barrier = watcher.runtime.dispatch.lock().await;
    let stopping = watcher.clone();
    let stop = tokio::spawn(async move {
        stopping.shutdown().await;
    });
    // Tokio's mutex queues the stop request before the debounced event dispatch.
    tokio::time::sleep(Duration::from_millis(30)).await;
    watcher.runtime.sender.send(signal).await.unwrap();
    tokio::time::sleep(Duration::from_millis(450)).await;
    assert!(!stop.is_finished());
    drop(barrier);
    stop.await.unwrap();
    assert!(!watcher.list().await.unwrap()[0].active);
    tokio::time::sleep(Duration::from_millis(650)).await;
    assert_eq!(task_count(&repo).await, 0);
    assert_eq!(
        watcher.configure(root.id, true).await.unwrap_err(),
        RepositoryError::Conflict
    );
}

#[tokio::test]
async fn restore_catches_up_changes_made_while_process_was_stopped_without_a_native_event() {
    let (_dir, repo, scans, watcher, root) = fixture().await;
    watcher.configure(root.id, true).await.unwrap();
    watcher.shutdown().await;
    // The previous watcher is stopped before this file exists.
    std::fs::write(
        Path::new(&root.path).join("Offline.2024.mkv"),
        b"offline fixture",
    )
    .unwrap();
    let restarted = WatcherService::new(repo.clone(), scans.clone());
    assert!(!restarted.list().await.unwrap()[0].active);
    restarted.restore().await.unwrap();
    wait_tasks(&repo, 1).await;
    let task = scans.list_tasks(false).await.unwrap().remove(0);
    settled(&scans, task.id).await;
    let preview = scans
        .preview(&PreviewInput {
            task_id: task.id,
            cursor: None,
            limit: 100,
            include_adult: false,
        })
        .await
        .unwrap();
    assert_eq!(preview.items.len(), 1);
    assert_eq!(preview.items[0].relative_path, "Offline.2024.mkv");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM media_items")
            .fetch_one(repo.pool())
            .await
            .unwrap(),
        0
    );
    restarted.shutdown().await;
}
#[tokio::test]
async fn replaced_or_missing_filesystem_root_retires_native_generation_and_can_recover() {
    let (dir, repo, _scans, watcher, root) = fixture().await;
    watcher.configure(root.id, true).await.unwrap();
    let old = watcher.runtime.live.lock().unwrap().handles[&root.id].signal;
    std::fs::rename(&root.path, dir.path().join("old-root")).unwrap();
    watcher.reconcile().await.unwrap();
    assert!(!watcher.list().await.unwrap()[0].active);
    std::fs::create_dir(&root.path).unwrap();
    watcher.reconcile().await.unwrap();
    let new = watcher.runtime.live.lock().unwrap().handles[&root.id].signal;
    assert_ne!(old, new);
    watcher.runtime.sender.send(old).await.unwrap();
    record_overflow(&watcher.runtime.overflow, old);
    tokio::time::sleep(Duration::from_millis(650)).await;
    assert_eq!(task_count(&repo).await, 0);
    watcher.shutdown().await;
}
