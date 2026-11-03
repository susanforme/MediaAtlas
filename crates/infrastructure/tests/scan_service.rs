mod support;

use chrono::Utc;
use mediaatlas_application::library::{ConfirmInput, PreviewInput, RootInput};
use mediaatlas_domain::{LibraryRoot, MediaKind, TaskState};
use mediaatlas_infrastructure::{
    browse::BrowseService,
    library::{LibraryRepository, ListMediaQuery, MetadataWriteMode, RepositoryError},
    scan_service::ScanService,
};
use serde_json::json;
use std::{path::Path, time::Duration};
use tempfile::TempDir;
use uuid::Uuid;

async fn setup(kind: Option<MediaKind>) -> (TempDir, LibraryRepository, ScanService, LibraryRoot) {
    let dir = support::tempdir().unwrap();
    let media = dir.path().join("media");
    std::fs::create_dir(&media).unwrap();
    let repo = LibraryRepository::open(&dir.path().join("library.sqlite"))
        .await
        .unwrap();
    let root = BrowseService::new(repo.clone(), dir.path().join("cache"))
        .save_root(&RootInput {
            id: None,
            name: "Fixture root".into(),
            path: media.to_str().unwrap().into(),
            kind,
            enabled: true,
            exclude_patterns: vec![],
        })
        .await
        .unwrap()
        .root;
    let service = ScanService::new(repo.clone());
    (dir, repo, service, root)
}

fn write(root: &LibraryRoot, name: &str, bytes: &[u8]) {
    let path = Path::new(&root.path).join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

async fn settled(service: &ScanService, id: Uuid) -> TaskState {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let view = service
                .list_tasks(true)
                .await
                .unwrap()
                .into_iter()
                .find(|view| view.id == id)
                .unwrap();
            if !matches!(view.state, TaskState::Queued | TaskState::Running) {
                return view.state;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("Fixture scan did not settle")
}

async fn preview(
    service: &ScanService,
    id: Uuid,
) -> Vec<mediaatlas_application::library::ScanCandidateView> {
    let mut cursor = None;
    let mut items = Vec::new();
    loop {
        let page = service
            .preview(&PreviewInput {
                task_id: id,
                cursor,
                limit: 100,
                include_adult: true,
            })
            .await
            .unwrap();
        items.extend(page.items);
        match page.next_cursor {
            Some(next) => cursor = Some(next),
            None => break,
        }
    }
    items
}

#[tokio::test]
async fn automatic_refresh_indexes_local_files_without_manual_import_or_provider_credentials() {
    let (_dir, repo, service, root) = setup(Some(MediaKind::Movie)).await;
    write(&root, "Unmatched.2020.mkv", b"source remains read-only");
    let task = service.start_automatic(root.id).await.unwrap();
    assert_eq!(settled(&service, task.id).await, TaskState::WaitingForUser);
    let tasks = service.list_tasks(false).await.unwrap();
    let task = tasks.iter().find(|t| t.id == task.id).unwrap();
    assert_eq!(task.mode, "automatic");
    assert_eq!(task.phase, "complete");
    assert_eq!(task.imported_count, 1);
    assert_eq!(task.review_count, 1);
    let page = BrowseService::new(repo.clone(), Path::new(&root.path).join("cache"))
        .list_media(&Default::default())
        .await
        .unwrap();
    assert_eq!(
        page.items.len(),
        1,
        "unmatched local files must already be in the library"
    );
    assert!(page.items[0]
        .health
        .contains(&"metadata_unconfigured".to_owned()));
    let candidates = preview(&service, task.id).await;
    assert_eq!(candidates[0].status, "imported");
    assert!(matches!(
        service
            .confirm(&ConfirmInput {
                task_id: task.id,
                candidate_ids: vec![candidates[0].id]
            })
            .await,
        Err(RepositoryError::Conflict)
    ));
    let next = service.start_automatic(root.id).await.unwrap();
    settled(&service, next.id).await;
    assert_eq!(
        repo.list_media(&ListMediaQuery::default())
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(
        std::fs::read(Path::new(&root.path).join("Unmatched.2020.mkv")).unwrap(),
        b"source remains read-only"
    );
    assert!(!Path::new(&root.path).join("Unmatched.2020.nfo").exists());
    repo.pool().close().await;
}

#[tokio::test]
async fn automatic_refresh_supersedes_legacy_waiting_scan_and_keeps_adult_manual() {
    let (_dir, repo, service, root) = setup(Some(MediaKind::Movie)).await;
    write(&root, "Example.2020.mkv", b"video");
    let legacy = service.start(root.id).await.unwrap();
    assert_eq!(
        settled(&service, legacy.id).await,
        TaskState::WaitingForUser
    );
    let next = service.start_automatic(root.id).await.unwrap();
    settled(&service, next.id).await;
    assert_eq!(
        service
            .list_tasks(false)
            .await
            .unwrap()
            .iter()
            .find(|t| t.id == legacy.id)
            .unwrap()
            .state,
        TaskState::Cancelled
    );
    assert_eq!(
        repo.list_media(&ListMediaQuery::default())
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    repo.pool().close().await;
    let (_dir, repo, service, root) = setup(Some(MediaKind::AdultVideo)).await;
    write(&root, "ABC-123.mkv", b"isolated");
    assert!(matches!(
        service.start_automatic(root.id).await,
        Err(RepositoryError::InvalidInput)
    ));
    assert!(service.list_tasks(true).await.unwrap().is_empty());
    assert!(repo
        .list_media(&ListMediaQuery::default())
        .await
        .unwrap()
        .items
        .is_empty());
    repo.pool().close().await;
}

#[tokio::test]
async fn automatic_refresh_recovery_marks_pending_metadata_interrupted_without_replaying() {
    let (_dir, repo, service, root) = setup(Some(MediaKind::Movie)).await;
    write(&root, "Example.2020.mkv", b"unchanged source");
    let task = service.start_automatic(root.id).await.unwrap();
    settled(&service, task.id).await;
    sqlx::query("UPDATE scrape_tasks SET state='running' WHERE id=?")
        .bind(task.id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE library_refresh_items SET state='pending',reason=NULL WHERE task_id=?")
        .bind(task.id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    service.recover_interrupted().await.unwrap();
    let recovered = service.list_tasks(false).await.unwrap().remove(0);
    assert_eq!(recovered.state, TaskState::Interrupted);
    assert_eq!(recovered.review_count, 1);
    let reason: String =
        sqlx::query_scalar("SELECT reason FROM library_refresh_items WHERE task_id=?")
            .bind(task.id.to_string())
            .fetch_one(repo.pool())
            .await
            .unwrap();
    assert_eq!(reason, "interrupted");
    assert_eq!(
        repo.list_media(&ListMediaQuery::default())
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert_eq!(
        std::fs::read(Path::new(&root.path).join("Example.2020.mkv")).unwrap(),
        b"unchanged source"
    );
    repo.pool().close().await;
}

#[tokio::test]
async fn scanning_stages_paginated_candidates_before_explicit_partial_confirmation() {
    let (_dir, repo, service, root) = setup(Some(MediaKind::Movie)).await;
    for name in ["One.2020.mkv", "Two.2021.mkv", "Three.2022.mkv"] {
        write(&root, name, b"fixture");
    }
    let task = service.start(root.id).await.unwrap();
    assert_eq!(settled(&service, task.id).await, TaskState::WaitingForUser);
    assert!(repo
        .list_media(&ListMediaQuery::default())
        .await
        .unwrap()
        .items
        .is_empty());
    let first = service
        .preview(&PreviewInput {
            task_id: task.id,
            cursor: None,
            limit: 1,
            include_adult: false,
        })
        .await
        .unwrap();
    assert_eq!(first.items.len(), 1);
    assert!(first.next_cursor.is_some());
    let result = service
        .confirm(&ConfirmInput {
            task_id: task.id,
            candidate_ids: vec![first.items[0].id],
        })
        .await
        .unwrap();
    assert_eq!(result.imported, 1);
    assert_eq!(settled(&service, task.id).await, TaskState::WaitingForUser);
    let repeated = service
        .confirm(&ConfirmInput {
            task_id: task.id,
            candidate_ids: vec![first.items[0].id],
        })
        .await
        .unwrap();
    assert_eq!(repeated.unchanged, 1);
    let rest = preview(&service, task.id)
        .await
        .into_iter()
        .filter(|v| v.status == "pending")
        .map(|v| v.id)
        .collect();
    assert_eq!(
        service
            .confirm(&ConfirmInput {
                task_id: task.id,
                candidate_ids: rest
            })
            .await
            .unwrap()
            .imported,
        2
    );
    assert_eq!(settled(&service, task.id).await, TaskState::Succeeded);
    assert_eq!(
        repo.list_media(&ListMediaQuery::default())
            .await
            .unwrap()
            .items
            .len(),
        3
    );
    let next = service.start(root.id).await.unwrap();
    assert_eq!(settled(&service, next.id).await, TaskState::Succeeded);
    assert!(preview(&service, next.id)
        .await
        .iter()
        .all(|v| v.status == "unchanged"));
    assert_eq!(
        std::fs::read(Path::new(&root.path).join("One.2020.mkv")).unwrap(),
        b"fixture"
    );
    repo.pool().close().await;
}

#[tokio::test]
async fn parts_share_a_work_and_tv_episodes_keep_series_and_season_relationships() {
    let (_dir, repo, service, root) = setup(Some(MediaKind::Movie)).await;
    write(&root, "Film.2020.CD1.mkv", b"part1");
    write(&root, "Film.2020.CD2.mkv", b"part2");
    let task = service.start(root.id).await.unwrap();
    settled(&service, task.id).await;
    service
        .confirm(&ConfirmInput {
            task_id: task.id,
            candidate_ids: preview(&service, task.id)
                .await
                .iter()
                .map(|v| v.id)
                .collect(),
        })
        .await
        .unwrap();
    let page = repo.list_media(&ListMediaQuery::default()).await.unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(repo.list_files(page.items[0].id).await.unwrap().len(), 2);
    repo.pool().close().await;
    let (_dir, repo, service, root) = setup(Some(MediaKind::TvSeries)).await;
    write(&root, "Show/Season 01/Show.S01E01.mkv", b"episode1");
    write(&root, "Show/Season 01/Show.S01E02.mkv", b"episode2");
    let task = service.start(root.id).await.unwrap();
    assert_eq!(settled(&service, task.id).await, TaskState::WaitingForUser);
    service
        .confirm(&ConfirmInput {
            task_id: task.id,
            candidate_ids: preview(&service, task.id)
                .await
                .iter()
                .map(|v| v.id)
                .collect(),
        })
        .await
        .unwrap();
    let all = repo
        .list_media(&ListMediaQuery::default())
        .await
        .unwrap()
        .items;
    assert_eq!(
        all.iter().filter(|v| v.kind == MediaKind::TvSeries).count(),
        1
    );
    assert_eq!(
        all.iter().filter(|v| v.kind == MediaKind::TvSeason).count(),
        1
    );
    assert_eq!(
        all.iter()
            .filter(|v| v.kind == MediaKind::TvEpisode)
            .count(),
        2
    );
    let links: i64 = sqlx::query_scalar("SELECT count(*) FROM media_relations")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    assert_eq!(links, 3);
    repo.pool().close().await;
}

#[tokio::test]
async fn stale_source_or_changed_root_policy_rejects_confirmation_atomically() {
    let (_dir, repo, service, root) = setup(Some(MediaKind::Movie)).await;
    write(&root, "One.mkv", b"original");
    write(&root, "Two.mkv", b"original");
    let task = service.start(root.id).await.unwrap();
    settled(&service, task.id).await;
    let candidates = preview(&service, task.id).await;
    write(
        &root,
        &candidates[1].relative_path,
        b"changed source length",
    );
    assert_eq!(
        service
            .confirm(&ConfirmInput {
                task_id: task.id,
                candidate_ids: candidates.iter().map(|v| v.id).collect()
            })
            .await
            .unwrap_err(),
        RepositoryError::Conflict
    );
    assert!(repo
        .list_media(&ListMediaQuery::default())
        .await
        .unwrap()
        .items
        .is_empty());
    sqlx::query("INSERT INTO root_settings(root_id,exclude_patterns_json) VALUES(?,?) ON CONFLICT(root_id) DO UPDATE SET exclude_patterns_json=excluded.exclude_patterns_json")
        .bind(root.id.to_string()).bind("[\"*.mkv\"]").execute(repo.pool()).await.unwrap();
    assert_eq!(
        service
            .confirm(&ConfirmInput {
                task_id: task.id,
                candidate_ids: vec![candidates[0].id]
            })
            .await
            .unwrap_err(),
        RepositoryError::Conflict
    );
    assert!(repo
        .list_media(&ListMediaQuery::default())
        .await
        .unwrap()
        .items
        .is_empty());
    repo.pool().close().await;
}

#[tokio::test]
async fn changed_sidecar_creates_a_pending_candidate_without_overwriting_locked_fields() {
    let (_dir, repo, service, root) = setup(Some(MediaKind::Movie)).await;
    write(&root, "Film.2020.mkv", b"fixture");
    write(
        &root,
        "Film.2020.nfo",
        b"<movie><title>Original local title</title><genre>Drama</genre></movie>",
    );
    let task = service.start(root.id).await.unwrap();
    settled(&service, task.id).await;
    service
        .confirm(&ConfirmInput {
            task_id: task.id,
            candidate_ids: preview(&service, task.id)
                .await
                .iter()
                .map(|v| v.id)
                .collect(),
        })
        .await
        .unwrap();
    let media = repo
        .list_media(&ListMediaQuery::default())
        .await
        .unwrap()
        .items
        .remove(0);
    let mut title = repo
        .list_metadata_values(media.id)
        .await
        .unwrap()
        .into_iter()
        .find(|v| v.field_key == "title")
        .unwrap();
    title.value = json!("User title");
    title.is_locked = true;
    repo.save_metadata_value(&title, MetadataWriteMode::User)
        .await
        .unwrap();
    let mut media = media;
    media.title = "User title".into();
    repo.save_media(&media).await.unwrap();
    write(
        &root,
        "Film.2020.nfo",
        b"<movie><title>New longer local title</title><genre>Comedy</genre></movie>",
    );
    let changed = service.start(root.id).await.unwrap();
    assert_eq!(
        settled(&service, changed.id).await,
        TaskState::WaitingForUser
    );
    let candidates = preview(&service, changed.id).await;
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].status, "pending");
    service
        .confirm(&ConfirmInput {
            task_id: changed.id,
            candidate_ids: vec![candidates[0].id],
        })
        .await
        .unwrap();
    assert_eq!(
        repo.get_media(media.id).await.unwrap().unwrap().title,
        "User title"
    );
    assert_eq!(
        repo.list_metadata_values(media.id)
            .await
            .unwrap()
            .into_iter()
            .find(|v| v.field_key == "title")
            .unwrap()
            .value,
        json!("User title")
    );
    repo.pool().close().await;
}

#[tokio::test]
async fn deleted_sidecars_are_reconciled_only_after_confirmation_without_removing_user_artwork() {
    let (dir, repo, service, root) = setup(Some(MediaKind::Movie)).await;
    write(&root, "Fixture.2020.mkv", b"fixture source");
    write(
        &root,
        "Fixture.2020.nfo",
        b"<movie><title>Fixture title</title></movie>",
    );
    image::RgbImage::from_pixel(40, 60, image::Rgb([10, 80, 30]))
        .save(Path::new(&root.path).join("poster.png"))
        .unwrap();
    let first = service.start(root.id).await.unwrap();
    settled(&service, first.id).await;
    service
        .confirm(&ConfirmInput {
            task_id: first.id,
            candidate_ids: preview(&service, first.id)
                .await
                .iter()
                .map(|v| v.id)
                .collect(),
        })
        .await
        .unwrap();
    let media = repo
        .list_media(&ListMediaQuery::default())
        .await
        .unwrap()
        .items
        .remove(0);
    let browse = BrowseService::new(repo.clone(), dir.path().join("cache"));
    let initial = browse
        .get_media(&mediaatlas_application::library::MediaDetailInput {
            media_id: media.id,
            include_adult: false,
        })
        .await
        .unwrap();
    assert!(!initial.health.contains(&"missing_nfo".to_owned()));
    assert!(!initial.health.contains(&"missing_poster".to_owned()));
    let manual = mediaatlas_domain::Artwork {
        id: Uuid::new_v4(),
        media_item_id: media.id,
        kind: mediaatlas_domain::ArtworkKind::Backdrop,
        cache_path: None,
        source_url: Some("https://example.invalid/user-selected.png".into()),
        provider_id: Some("user".into()),
        width: None,
        height: None,
        content_hash: None,
        created_at: Utc::now(),
    };
    repo.save_artwork(&manual).await.unwrap();
    std::fs::remove_file(Path::new(&root.path).join("Fixture.2020.nfo")).unwrap();
    std::fs::remove_file(Path::new(&root.path).join("poster.png")).unwrap();
    let changed = service.start(root.id).await.unwrap();
    assert_eq!(
        settled(&service, changed.id).await,
        TaskState::WaitingForUser
    );
    // Scanning is a preview and does not remove indexed data before confirmation.
    assert!(repo
        .list_files(media.id)
        .await
        .unwrap()
        .iter()
        .any(|file| file.relative_path.ends_with(".nfo")));
    let pending = preview(&service, changed.id).await;
    service
        .confirm(&ConfirmInput {
            task_id: changed.id,
            candidate_ids: pending.iter().map(|v| v.id).collect(),
        })
        .await
        .unwrap();
    let detail = browse
        .get_media(&mediaatlas_application::library::MediaDetailInput {
            media_id: media.id,
            include_adult: false,
        })
        .await
        .unwrap();
    assert!(detail.health.contains(&"missing_nfo".to_owned()));
    assert!(detail.health.contains(&"missing_poster".to_owned()));
    assert_eq!(detail.files.len(), 1);
    assert_eq!(detail.artworks.len(), 1);
    assert_eq!(detail.artworks[0].id, manual.id);
    assert_eq!(
        std::fs::read(Path::new(&root.path).join("Fixture.2020.mkv")).unwrap(),
        b"fixture source"
    );
    repo.pool().close().await;
}

#[tokio::test]
async fn cancellation_and_restart_recovery_do_not_import_or_replay_candidates() {
    let (_dir, repo, service, root) = setup(Some(MediaKind::Movie)).await;
    for i in 0..200 {
        write(&root, &format!("Film{i}.mkv"), b"fixture");
    }
    let task = service.start(root.id).await.unwrap();
    service.cancel(task.id).await.unwrap();
    assert_eq!(settled(&service, task.id).await, TaskState::Cancelled);
    assert!(repo
        .list_media(&ListMediaQuery::default())
        .await
        .unwrap()
        .items
        .is_empty());
    let now = Utc::now().timestamp_millis();
    let interrupted = Uuid::new_v4();
    sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES(?,'scan_library','running','{}',?,?)")
        .bind(interrupted.to_string()).bind(now).bind(now).execute(repo.pool()).await.unwrap();
    sqlx::query("INSERT INTO scan_sessions(task_id,root_id,root_snapshot_json) VALUES(?,?,?)")
        .bind(interrupted.to_string())
        .bind(root.id.to_string())
        .bind(json!({"root":root,"exclude_patterns":[]}).to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    ScanService::new(repo.clone())
        .recover_interrupted()
        .await
        .unwrap();
    assert!(service
        .list_tasks(true)
        .await
        .unwrap()
        .iter()
        .any(|v| v.id == interrupted && v.state == TaskState::Interrupted));
    assert!(repo
        .list_media(&ListMediaQuery::default())
        .await
        .unwrap()
        .items
        .is_empty());
    tokio::time::sleep(Duration::from_millis(30)).await;
    repo.pool().close().await;
}

#[tokio::test]
async fn adult_and_mixed_sessions_do_not_leak_counters_in_default_task_reads() {
    let (_dir, repo, service, root) = setup(Some(MediaKind::AdultVideo)).await;
    write(&root, "ABC-123.mkv", b"fixture");
    let task = service.start(root.id).await.unwrap();
    settled(&service, task.id).await;
    assert!(service.list_tasks(false).await.unwrap().is_empty());
    assert_eq!(
        service
            .preview(&PreviewInput {
                task_id: task.id,
                cursor: None,
                limit: 20,
                include_adult: false
            })
            .await
            .unwrap_err(),
        RepositoryError::NotFound
    );
    // A Root can later become Mixed. Its existing adult candidate still hides the session.
    sqlx::query("UPDATE library_roots SET kind=NULL WHERE id=?")
        .bind(root.id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(service.list_tasks(false).await.unwrap().is_empty());
    assert_eq!(
        service.list_tasks(true).await.unwrap()[0].candidate_count,
        1
    );
    repo.pool().close().await;
    // The same visibility boundary applies to a real Mixed-root scan, without
    // hand-editing candidate kinds in SQLite.
    let (_dir, repo, service, root) = setup(None).await;
    write(&root, "ABC-123.mkv", b"fixture adult");
    write(&root, "Ordinary.2020.mkv", b"fixture ordinary");
    let task = service.start(root.id).await.unwrap();
    assert_eq!(settled(&service, task.id).await, TaskState::WaitingForUser);
    assert!(service.list_tasks(false).await.unwrap().is_empty());
    assert_eq!(
        service.list_tasks(true).await.unwrap()[0].candidate_count,
        2
    );
    let ordinary = service
        .preview(&PreviewInput {
            task_id: task.id,
            cursor: None,
            limit: 20,
            include_adult: false,
        })
        .await
        .unwrap();
    assert_eq!(ordinary.items.len(), 1);
    assert_eq!(ordinary.items[0].kind, MediaKind::Movie);
    let all = preview(&service, task.id).await;
    assert!(all
        .iter()
        .any(|view| view.kind == MediaKind::AdultVideo && view.needs_review));
    repo.pool().close().await;
}

#[tokio::test]
async fn confirmation_rejects_duplicate_foreign_and_excessive_selection_and_cursor_reuse() {
    let (_dir, repo, service, root) = setup(Some(MediaKind::Movie)).await;
    write(&root, "Fixture.mkv", b"fixture");
    let first = service.start(root.id).await.unwrap();
    settled(&service, first.id).await;
    let id = preview(&service, first.id).await[0].id;
    for ids in [vec![], vec![id, id], vec![id; 201]] {
        assert_eq!(
            service
                .confirm(&ConfirmInput {
                    task_id: first.id,
                    candidate_ids: ids
                })
                .await
                .unwrap_err(),
            RepositoryError::InvalidInput
        );
    }
    assert_eq!(
        service
            .confirm(&ConfirmInput {
                task_id: first.id,
                candidate_ids: vec![Uuid::new_v4()]
            })
            .await
            .unwrap_err(),
        RepositoryError::NotFound
    );
    let fake = json!({"task_id":Uuid::new_v4(),"include_adult":true,"after":id}).to_string();
    assert_eq!(
        service
            .preview(&PreviewInput {
                task_id: first.id,
                cursor: Some(fake),
                limit: 10,
                include_adult: true
            })
            .await
            .unwrap_err(),
        RepositoryError::InvalidInput
    );
    repo.pool().close().await;
}

#[cfg(unix)]
#[tokio::test]
async fn source_changed_to_symlink_is_rejected_before_import() {
    let (dir, repo, service, root) = setup(Some(MediaKind::Movie)).await;
    write(&root, "Fixture.mkv", b"fixture");
    let task = service.start(root.id).await.unwrap();
    settled(&service, task.id).await;
    let id = preview(&service, task.id).await[0].id;
    let outside = dir.path().join("outside.mkv");
    std::fs::write(&outside, b"fixture").unwrap();
    let source = Path::new(&root.path).join("Fixture.mkv");
    std::fs::remove_file(&source).unwrap();
    std::os::unix::fs::symlink(&outside, &source).unwrap();
    assert_eq!(
        service
            .confirm(&ConfirmInput {
                task_id: task.id,
                candidate_ids: vec![id]
            })
            .await
            .unwrap_err(),
        RepositoryError::Conflict
    );
    assert!(repo
        .list_media(&ListMediaQuery::default())
        .await
        .unwrap()
        .items
        .is_empty());
    assert_eq!(std::fs::read(outside).unwrap(), b"fixture");
    repo.pool().close().await;
}
