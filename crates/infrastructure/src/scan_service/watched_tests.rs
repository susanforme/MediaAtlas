use super::*;
use crate::browse::BrowseService;
use mediaatlas_application::library::RootInput;

async fn fixture() -> (tempfile::TempDir, LibraryRepository, ScanService, LibraryRoot) {
    let directory = crate::test_support::tempdir().unwrap();
    let media = directory.path().join("media");
    fs::create_dir(&media).unwrap();
    let repo = LibraryRepository::open(&directory.path().join("library.sqlite")).await.unwrap();
    let root = BrowseService::new(repo.clone(), directory.path().join("cache")).save_root(&RootInput {
        id: None, name: "Selected sources".into(), path: media.to_string_lossy().into_owned(),
        kind: Some(MediaKind::Movie), enabled: true, exclude_patterns: vec!["ignored/**".into()],
        metadata_providers: Some(vec![]), nfo_export_mode: None,
    }).await.unwrap().root;
    let scans = ScanService::new(repo.clone());
    (directory, repo, scans, root)
}

fn write(root: &LibraryRoot, relative: &str, bytes: &[u8]) -> PathBuf {
    let path = Path::new(&root.path).join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, bytes).unwrap();
    path
}

async fn settle(scans: &ScanService, id: Uuid) -> ScanTaskView {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let task = scans.get_task(id).await.unwrap();
            if !matches!(task.state, TaskState::Queued | TaskState::Running) { return task; }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await.expect("selected scan did not settle")
}

async fn with_automatic_scraper(scans: ScanService, repo: &LibraryRepository, cache: &Path) -> ScanService {
    let providers = crate::providers::ProviderService::with_engine(repo.clone(), crate::network::NetworkEngine::for_loopback(1)).await.unwrap();
    let engine = providers.engine();
    let hub = Arc::new(crate::video_providers::VideoProviderHub::new(engine.clone(), Arc::new(crate::credentials::CredentialStore::new())));
    scans.with_scraper(crate::scraper::ScraperService::new(repo.clone(), providers, hub, engine, cache.to_path_buf()))
}

#[tokio::test]
async fn sidecar_selects_only_its_actual_source_including_an_unindexed_file() {
    let (_directory, repo, scans, root) = fixture().await;
    write(&root, "One.2020.mkv", b"one source");
    write(&root, "Two.2021.mkv", b"unrelated source");
    let sidecar = write(&root, "One.2020.nfo", b"<movie><title>Local One</title></movie>");
    let task = scans.start_watched_paths(&root, &[sidecar]).await.unwrap().unwrap();
    let task = settle(&scans, task.id).await;
    assert_eq!(task.visited_files, 1);
    assert_eq!(task.candidate_count, 1);
    assert_eq!(scans.saved_snapshot(task.id).await.unwrap().selected_sources, Some(vec!["One.2020.mkv".into()]));
    let page = scans.preview(&PreviewInput { task_id: task.id, cursor: None, limit: 100, include_adult: false }).await.unwrap();
    assert_eq!(page.items[0].relative_path, "One.2020.mkv");
    scans.confirm(&ConfirmInput { task_id: task.id, candidate_ids: vec![page.items[0].id] }).await.unwrap();
    let titles: Vec<String> = sqlx::query_scalar("SELECT title FROM media_items").fetch_all(repo.pool()).await.unwrap();
    assert_eq!(titles, vec!["Local One"]);
    assert_eq!(fs::read(Path::new(&root.path).join("Two.2021.mkv")).unwrap(), b"unrelated source");
    repo.pool().close().await;
}

#[tokio::test]
async fn exact_video_events_force_a_pending_candidate_without_cancelling_unrelated_review() {
    let (_directory, repo, scans, root) = fixture().await;
    let first_path = write(&root, "One.2020.mkv", b"one");
    write(&root, "Two.2021.mkv", b"two");
    let initial = scans.start_automatic(root.id).await.unwrap();
    assert_eq!(settle(&scans, initial.id).await.state, TaskState::WaitingForUser);
    let task = scans.start_watched_paths(&root, &[first_path]).await.unwrap().unwrap();
    let task = settle(&scans, task.id).await;
    assert_eq!(task.visited_files, 1);
    let page = scans.preview(&PreviewInput { task_id: task.id, cursor: None, limit: 100, include_adult: false }).await.unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].status, "pending");
    assert_eq!(scans.get_task(initial.id).await.unwrap().state, TaskState::WaitingForUser);
    assert_eq!(scans.get_task(initial.id).await.unwrap().review_count, 2);
    repo.pool().close().await;
}

#[tokio::test]
async fn deleted_video_only_removes_its_source_index_and_retains_metadata_and_other_files() {
    let (directory, repo, scans, root) = fixture().await;
    let scans = with_automatic_scraper(scans, &repo, &directory.path().join("cache")).await;
    let removed_path = write(&root, "One.2020.mkv", b"one");
    write(&root, "One.2020.nfo", b"<movie><title>Saved One</title><uniqueid type=\"imdb\">tt12345</uniqueid></movie>");
    write(&root, "Two.2021.mkv", b"two");
    let initial = scans.start_automatic(root.id).await.unwrap();
    settle(&scans, initial.id).await;
    let media_id: String = sqlx::query_scalar("SELECT media_item_id FROM media_files WHERE root_id=? AND relative_path='One.2020.mkv'")
        .bind(root.id.to_string()).fetch_one(repo.pool()).await.unwrap();
    fs::remove_file(&removed_path).unwrap();
    let task = scans.start_watched_paths(&root, &[removed_path]).await.unwrap().unwrap();
    let task = settle(&scans, task.id).await;
    assert_eq!(task.state, TaskState::Succeeded);
    let remaining: Vec<String> = sqlx::query_scalar("SELECT relative_path FROM media_files WHERE root_id=? ORDER BY relative_path")
        .bind(root.id.to_string()).fetch_all(repo.pool()).await.unwrap();
    assert!(!remaining.contains(&"One.2020.mkv".to_owned()));
    assert!(remaining.contains(&"One.2020.nfo".to_owned()));
    assert!(remaining.contains(&"Two.2021.mkv".to_owned()));
    let media_id = Uuid::parse_str(&media_id).unwrap();
    assert_eq!(repo.get_media(media_id).await.unwrap().unwrap().title, "Saved One");
    assert!(repo.list_external_ids(media_id).await.unwrap().iter().any(|id| id.external_id == "tt12345"));
    let signatures: i64 = sqlx::query_scalar("SELECT count(*) FROM scan_source_fingerprints WHERE root_id=? AND normalized_relative_path='One.2020.mkv'")
        .bind(root.id.to_string()).fetch_one(repo.pool()).await.unwrap();
    assert_eq!(signatures, 0);
    repo.pool().close().await;
}

#[tokio::test]
async fn selected_review_results_preserve_unrelated_and_still_failed_items() {
    let (_directory, repo, scans, root) = fixture().await;
    let path = write(&root, "One.2020.mkv", b"one");
    write(&root, "Two.2021.mkv", b"two");
    let initial = scans.start_automatic(root.id).await.unwrap();
    settle(&scans, initial.id).await;
    let media: String = sqlx::query_scalar("SELECT media_item_id FROM media_files WHERE root_id=? AND relative_path='One.2020.mkv'")
        .bind(root.id.to_string()).fetch_one(repo.pool()).await.unwrap();
    let media = Uuid::parse_str(&media).unwrap();
    let current = scans.start_watched_paths(&root, &[path]).await.unwrap().unwrap();
    settle(&scans, current.id).await;
    sqlx::query("INSERT INTO library_refresh_items(task_id,media_item_id,state,reason,updated_at) VALUES(?,?,'failed','metadata_failed',?)")
        .bind(current.id.to_string()).bind(media.to_string()).bind(Utc::now().timestamp_millis()).execute(repo.pool()).await.unwrap();
    scans.reconcile_selected_reviews(current.id, root.id, media).await.unwrap();
    let old = scans.get_task(initial.id).await.unwrap();
    assert_eq!(old.state, TaskState::WaitingForUser);
    assert_eq!(old.review_count, 1);
    assert_eq!(old.metadata_failed_count, 1);
    sqlx::query("UPDATE library_refresh_items SET state='skipped',reason='existing_metadata' WHERE task_id=?")
        .bind(current.id.to_string()).execute(repo.pool()).await.unwrap();
    scans.reconcile_selected_reviews(current.id, root.id, media).await.unwrap();
    let old = scans.get_task(initial.id).await.unwrap();
    assert_eq!(old.state, TaskState::WaitingForUser);
    assert_eq!(old.review_count, 1);
    assert_eq!(old.metadata_failed_count, 0);
    repo.pool().close().await;
}

#[tokio::test]
async fn private_excluded_and_empty_selections_never_expand_to_a_root_scan() {
    let (_directory, repo, scans, root) = fixture().await;
    let ignored = write(&root, "ignored/Hidden.mkv", b"ignored");
    let private = write(&root, ".mediaatlas-work/Hidden.mkv", b"private");
    write(&root, "Visible.mkv", b"visible");
    assert!(scans.start_watched_paths(&root, &[ignored, private]).await.unwrap().is_none());
    assert!(scans.start_watched_paths(&root, &[]).await.unwrap().is_none());
    let empty = scans.start_inner(root.id, None, false, None, Some(vec![])).await.unwrap();
    let empty = settle(&scans, empty.id).await;
    assert_eq!(empty.visited_files, 0);
    assert_eq!(empty.candidate_count, 0);
    repo.pool().close().await;
}

#[tokio::test]
async fn missing_parent_is_not_a_deleted_source_and_outside_events_are_rejected() {
    let (directory, repo, scans, root) = fixture().await;
    let source = write(&root, "Collection/One.mkv", b"one");
    let first = scans.start_automatic(root.id).await.unwrap();
    settle(&scans, first.id).await;
    fs::rename(Path::new(&root.path).join("Collection"), directory.path().join("detached")).unwrap();
    let task = scans.start_watched_paths(&root, &[source]).await.unwrap().unwrap();
    settle(&scans, task.id).await;
    assert!(scans.saved_snapshot(task.id).await.unwrap().selected_sources.is_none());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM media_files WHERE root_id=? AND relative_path='Collection/One.mkv'")
        .bind(root.id.to_string()).fetch_one(repo.pool()).await.unwrap();
    assert_eq!(count, 1);
    assert_eq!(scans.start_watched_paths(&root, &[directory.path().join("outside.mkv")]).await.unwrap_err(), RepositoryError::InvalidInput);
    repo.pool().close().await;
}

#[tokio::test]
async fn shared_series_and_actor_sidecars_stay_within_their_source_directory() {
    let (_directory, repo, scans, root) = fixture().await;
    write(&root, "Show/Season 01/Show.S01E01.mkv", b"episode one");
    write(&root, "Show/Season 02/Show.S02E01.mkv", b"episode two");
    write(&root, "Other/Unrelated.mkv", b"unrelated");
    let sidecar = write(&root, "Show/tvshow.nfo", b"<tvshow><title>Show</title></tvshow>");
    let selected = scans.start_watched_paths(&root, &[sidecar]).await.unwrap().unwrap();
    settle(&scans, selected.id).await;
    assert_eq!(scans.saved_snapshot(selected.id).await.unwrap().selected_sources, Some(vec![
        "Show/Season 01/Show.S01E01.mkv".into(), "Show/Season 02/Show.S02E01.mkv".into(),
    ]));
    scans.cancel(selected.id).await.unwrap();
    write(&root, "Movie/First.mkv", b"first");
    write(&root, "Movie/Second.mkv", b"second");
    let actor = write(&root, "Movie/.actors/Person.jpg", b"actor");
    let actor = scans.start_watched_paths(&root, &[actor]).await.unwrap().unwrap();
    settle(&scans, actor.id).await;
    assert_eq!(scans.saved_snapshot(actor.id).await.unwrap().selected_sources, Some(vec!["Movie/First.mkv".into(), "Movie/Second.mkv".into()]));
    repo.pool().close().await;
}

#[tokio::test]
async fn deleted_source_beneath_an_isolated_parent_keeps_its_index() {
    let (directory, repo, scans, root) = fixture().await;
    let scans = with_automatic_scraper(scans, &repo, &directory.path().join("cache")).await;
    let path = write(&root, "One.mkv", b"source");
    let initial = scans.start_automatic(root.id).await.unwrap();
    settle(&scans, initial.id).await;
    let child: String = sqlx::query_scalar("SELECT media_item_id FROM media_files WHERE root_id=? AND relative_path='One.mkv'")
        .bind(root.id.to_string()).fetch_one(repo.pool()).await.unwrap();
    let mut parent = repo.get_media(Uuid::parse_str(&child).unwrap()).await.unwrap().unwrap();
    parent.id = Uuid::new_v4();
    parent.kind = MediaKind::AdultVideo;
    repo.save_media(&parent).await.unwrap();
    sqlx::query("INSERT INTO media_relations(parent_id,child_id,relation_type) VALUES(?,?,'episode')")
        .bind(parent.id.to_string()).bind(child).execute(repo.pool()).await.unwrap();
    fs::remove_file(&path).unwrap();
    let task = scans.start_watched_paths(&root, &[path]).await.unwrap().unwrap();
    assert_eq!(settle(&scans, task.id).await.state, TaskState::Failed);
    let remaining: i64 = sqlx::query_scalar("SELECT count(*) FROM media_files WHERE root_id=? AND relative_path='One.mkv'")
        .bind(root.id.to_string()).fetch_one(repo.pool()).await.unwrap();
    assert_eq!(remaining, 1);
    repo.pool().close().await;
}

#[tokio::test]
async fn legacy_manual_watcher_never_removes_a_deleted_source_index() {
    let (_directory, repo, scans, root) = fixture().await;
    let path = write(&root, "One.mkv", b"source");
    let initial = scans.start_automatic(root.id).await.unwrap();
    settle(&scans, initial.id).await;
    fs::remove_file(&path).unwrap();
    let selected = scans.start_watched_paths(&root, &[path]).await.unwrap().unwrap();
    settle(&scans, selected.id).await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM media_files WHERE root_id=? AND relative_path='One.mkv'")
        .bind(root.id.to_string()).fetch_one(repo.pool()).await.unwrap();
    assert_eq!(count, 1);
    repo.pool().close().await;
}
