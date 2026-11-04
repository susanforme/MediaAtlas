use super::*;
use mediaatlas_domain::{LibraryRoot, MediaFile, MediaItem};
use serde_json::json;

async fn setup() -> (
    tempfile::TempDir,
    LibraryRepository,
    ReviewQueueService,
    LibraryRoot,
    Vec<Uuid>,
) {
    let dir = crate::test_support::tempdir().unwrap();
    let repo = LibraryRepository::open(&dir.path().join("ledger.sqlite"))
        .await
        .unwrap();
    let now = Utc::now();
    let root = LibraryRoot {
        id: Uuid::new_v4(),
        name: "Synthetic ordinary folder".into(),
        path: dir.path().to_str().unwrap().into(),
        path_style: crate::test_support::native_path_style(),
        kind: Some(MediaKind::Movie),
        enabled: true,
        created_at: now,
        updated_at: now,
    };
    repo.save_root(&root).await.unwrap();
    let mut ids = vec![];
    for title in ["First", "Second"] {
        let id = Uuid::new_v4();
        repo.save_media(&MediaItem {
            id,
            kind: MediaKind::Movie,
            title: title.into(),
            original_title: None,
            year: Some(2020),
            created_at: now,
            updated_at: now,
        })
        .await
        .unwrap();
        std::fs::write(
            dir.path().join(format!("{title}.mkv")),
            b"read-only synthetic media",
        )
        .unwrap();
        repo.save_file(&MediaFile {
            id: Uuid::new_v4(),
            media_item_id: id,
            root_id: root.id,
            relative_path: format!("{title}.mkv"),
            size_bytes: 25,
            modified_at: None,
            part: None,
        })
        .await
        .unwrap();
        ids.push(id);
    }
    let service = ReviewQueueService::new(repo.clone());
    (dir, repo, service, root, ids)
}
fn create(ids: Vec<Uuid>) -> ReviewQueueCreate {
    ReviewQueueCreate {
        media_ids: ids,
        context: ReviewContext::default(),
    }
}
fn update(view: &ReviewQueueView, media: Uuid, action: ReviewAction) -> ReviewQueueUpdate {
    ReviewQueueUpdate {
        queue_id: view.id,
        media_id: media,
        expected_revision: view.revision,
        action,
        checkpoint: None,
    }
}

#[tokio::test]
async fn return_context_preserves_real_tools_pagination_and_rejects_other_scopes() {
    let (dir, repo, service, root, ids) = setup().await;
    let browse = crate::browse::BrowseService::new(repo.clone(), dir.path().join("cache"));
    let query = BrowseQuery {
        root_id: Some(root.id),
        kind: Some(MediaKind::Movie),
        limit: 1,
        ..Default::default()
    };
    let page = browse.tools_list_media(&query).await.unwrap();
    let cursor = page.next_cursor.unwrap();
    let context = ReviewContext {
        root: Some(root.id),
        kind: Some(MediaKind::Movie),
        cursor: Some(cursor.clone()),
        page: Some(2),
        ..Default::default()
    };
    let queued = service
        .create(ReviewQueueCreate {
            media_ids: ids.clone(),
            context: context.clone(),
        })
        .await
        .unwrap();
    assert_eq!(
        service.get(queued.id).await.unwrap().context.cursor,
        Some(cursor)
    );
    assert_eq!(queued.context.page, Some(2));
    let wall = browse
        .list_media(&query)
        .await
        .unwrap()
        .next_cursor
        .unwrap();
    let invalid = ReviewContext {
        cursor: Some(wall),
        ..context
    };
    assert!(matches!(
        service
            .create(ReviewQueueCreate {
                media_ids: ids,
                context: invalid
            })
            .await,
        Err(RepositoryError::InvalidInput)
    ));
    repo.pool().close().await;
}

#[tokio::test]
async fn typed_local_series_is_reviewable_but_an_isolated_episode_blocks_it() {
    let (_dir, repo, service, root, ids) = setup().await;
    sqlx::query("UPDATE media_items SET kind='tv_series' WHERE id=?")
        .bind(ids[0].to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query("DELETE FROM media_files WHERE media_item_id=?")
        .bind(ids[0].to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE media_items SET kind='tv_episode' WHERE id=?")
        .bind(ids[1].to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE library_roots SET kind='tv_series' WHERE id=?")
        .bind(root.id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO media_relations(parent_id,child_id,relation_type) VALUES(?,?,'episode')",
    )
    .bind(ids[0].to_string())
    .bind(ids[1].to_string())
    .execute(repo.pool())
    .await
    .unwrap();
    let mut input = create(vec![ids[0]]);
    input.context.root = Some(root.id);
    input.context.kind = Some(MediaKind::TvSeries);
    let view = service.create(input.clone()).await.unwrap();
    assert!(!view.items[0].needs_recheck);
    sqlx::query("UPDATE media_items SET adult_isolated=1 WHERE id=?")
        .bind(ids[1].to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    assert_eq!(
        service.get(view.id).await.unwrap().items[0]
            .reason
            .as_deref(),
        Some("unavailable_media")
    );
    assert!(matches!(
        service.create(input).await,
        Err(RepositoryError::InvalidInput)
    ));
    repo.pool().close().await;
}

#[tokio::test]
async fn ordered_ledger_survives_reopen_without_confirming_or_replaying() {
    let (dir, repo, service, root, ids) = setup().await;
    let mut input = create(vec![ids[1], ids[0], ids[1]]);
    input.context.root = Some(root.id);
    input.context.health = Some("metadata_needs_review".into());
    input.context.q = Some("Synthetic search".into());
    let view = service.create(input).await.unwrap();
    assert_eq!(
        view.items.iter().map(|i| i.media_id).collect::<Vec<_>>(),
        vec![ids[1], ids[0]]
    );
    let deferred = service
        .update(update(&view, ids[1], ReviewAction::Defer))
        .await
        .unwrap();
    assert_eq!(deferred.current_media_id, Some(ids[0]));
    let failed = service
        .update(update(&deferred, ids[0], ReviewAction::Failed))
        .await
        .unwrap();
    assert_eq!(failed.current_media_id, None);
    repo.pool().close().await;
    let reopened = LibraryRepository::open(&dir.path().join("ledger.sqlite"))
        .await
        .unwrap();
    let service = ReviewQueueService::new(reopened.clone());
    let restored = service.get(view.id).await.unwrap();
    assert_eq!(
        restored.items.iter().map(|i| i.state).collect::<Vec<_>>(),
        vec![ReviewState::Deferred, ReviewState::Failed]
    );
    assert_eq!(restored.context.root, Some(root.id));
    assert_eq!(restored.context.q.as_deref(), Some("Synthetic search"));
    let payload = serde_json::to_value(&restored).unwrap();
    assert!(payload["context"].get("kind").is_none());
    assert!(payload["items"][0].get("title").is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM video_merge_previews")
            .fetch_one(reopened.pool())
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        std::fs::read(dir.path().join("First.mkv")).unwrap(),
        b"read-only synthetic media"
    );
    let retry = service
        .update(update(&restored, ids[1], ReviewAction::Retry))
        .await
        .unwrap();
    assert_eq!(retry.current_media_id, Some(ids[1]));
    assert_eq!(retry.items[0].state, ReviewState::Pending);
    reopened.pool().close().await;
}

#[tokio::test]
async fn stale_and_concurrent_mutations_cannot_overwrite_current_queue() {
    let (_dir, repo, service, _root, ids) = setup().await;
    let view = service.create(create(ids.clone())).await.unwrap();
    let (first, second) = tokio::join!(
        service.update(update(&view, ids[0], ReviewAction::Defer)),
        service.update(update(&view, ids[0], ReviewAction::Failed))
    );
    assert_eq!(usize::from(first.is_ok()) + usize::from(second.is_ok()), 1);
    assert!(
        matches!(first, Err(RepositoryError::Conflict))
            || matches!(second, Err(RepositoryError::Conflict))
    );
    assert!(matches!(
        service
            .update(update(&view, ids[0], ReviewAction::Select))
            .await,
        Err(RepositoryError::Conflict)
    ));
    let current = service.get(view.id).await.unwrap();
    assert_eq!(current.revision, 1);
    assert_eq!(current.current_media_id, Some(ids[1]));
    repo.pool().close().await;
}

#[tokio::test]
async fn changed_roots_deleted_media_and_adult_ancestry_require_fresh_review() {
    let (_dir, repo, service, root, ids) = setup().await;
    let mut input = create(ids.clone());
    input.context.q = Some("Previously ordinary title".into());
    let view = service.create(input).await.unwrap();
    sqlx::query("UPDATE library_roots SET enabled=0 WHERE id=?")
        .bind(root.id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    let disabled = service.get(view.id).await.unwrap();
    assert!(disabled.context.q.is_none());
    assert!(disabled
        .items
        .iter()
        .all(|i| i.reason.as_deref() == Some("unavailable_media")));
    assert!(matches!(
        service
            .update(update(&disabled, ids[0], ReviewAction::Select))
            .await,
        Err(RepositoryError::Conflict)
    ));
    sqlx::query("UPDATE library_roots SET enabled=1,path=path||'/changed' WHERE id=?")
        .bind(root.id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    assert_eq!(
        service.get(view.id).await.unwrap().items[0]
            .reason
            .as_deref(),
        Some("expired_item")
    );
    let selected = service
        .update(update(&view, ids[0], ReviewAction::Select))
        .await
        .unwrap();
    assert!(!selected.items[0].needs_recheck);
    let adult = Uuid::new_v4();
    let now = Utc::now();
    repo.save_media(&MediaItem {
        id: adult,
        kind: MediaKind::AdultVideo,
        title: "Private fixture title".into(),
        original_title: None,
        year: None,
        created_at: now,
        updated_at: now,
    })
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO media_relations(parent_id,child_id,relation_type) VALUES(?,?,'episode')",
    )
    .bind(adult.to_string())
    .bind(ids[0].to_string())
    .execute(repo.pool())
    .await
    .unwrap();
    let isolated = service.get(view.id).await.unwrap();
    assert!(isolated.context.q.is_none());
    assert_eq!(
        isolated.items[0].reason.as_deref(),
        Some("unavailable_media")
    );
    assert!(!serde_json::to_string(&isolated)
        .unwrap()
        .contains("Private fixture title"));
    assert!(matches!(
        service.create(create(vec![ids[0]])).await,
        Err(RepositoryError::InvalidInput)
    ));
    sqlx::query("DELETE FROM media_items WHERE id=?")
        .bind(ids[1].to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    assert_eq!(
        service.get(view.id).await.unwrap().items[1]
            .reason
            .as_deref(),
        Some("missing_media")
    );
    repo.pool().close().await;
}

#[tokio::test]
async fn expired_pending_items_only_resume_after_explicit_recheck() {
    let (_dir, repo, service, _root, ids) = setup().await;
    let view = service.create(create(ids.clone())).await.unwrap();
    sqlx::query("UPDATE review_queue_items SET checked_at=0 WHERE queue_id=?")
        .bind(view.id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    let expired = service.get(view.id).await.unwrap();
    assert!(expired
        .items
        .iter()
        .all(|i| i.reason.as_deref() == Some("expired_item")));
    let resumed = service
        .update(update(&expired, ids[0], ReviewAction::Select))
        .await
        .unwrap();
    assert!(!resumed.items[0].needs_recheck);
    assert!(resumed.items[1].needs_recheck);
    assert_eq!(resumed.items[0].state, ReviewState::Pending);
    repo.pool().close().await;
}

#[tokio::test]
async fn queue_inputs_reject_unknown_secrets_remote_ids_and_oversized_selection() {
    let (_dir, repo, service, _root, ids) = setup().await;
    assert!(serde_json::from_value::<ReviewContext>(json!({"token":"never-store"})).is_err());
    for value in [
        json!({"q":"https://example.test/?token=secret"}),
        json!({"q":"Bearer secret"}),
        json!({"kind":"adult_video"}),
        json!({"health":"unknown"}),
        json!({"cursor":"https://example.test"}),
    ] {
        let context: ReviewContext = serde_json::from_value(value).unwrap();
        assert!(matches!(
            service
                .create(ReviewQueueCreate {
                    media_ids: ids.clone(),
                    context
                })
                .await,
            Err(RepositoryError::InvalidInput)
        ));
    }
    assert!(matches!(
        service.create(create(vec![ids[0]; 33])).await,
        Err(RepositoryError::InvalidInput)
    ));
    assert!(matches!(
        service.create(create(vec![Uuid::new_v4()])).await,
        Err(RepositoryError::InvalidInput)
    ));
    assert!(service.list().await.unwrap().is_empty());
    repo.pool().close().await;
}

#[tokio::test]
async fn preview_watermark_distinguishes_old_and_new_previews_with_identical_milliseconds() {
    let (_dir, repo, service, _root, ids) = setup().await;
    let task = Uuid::new_v4();
    let before = Uuid::new_v4();
    let now = Utc::now().timestamp_millis();
    sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) VALUES(?,'search_providers','waiting_for_user','{}',?,?)")
        .bind(task.to_string()).bind(now).bind(now).execute(repo.pool()).await.unwrap();
    sqlx::query("INSERT INTO video_scrape_sessions(task_id,query_json,jobs_json,created_at) VALUES(?,'{}','{}',?)")
        .bind(task.to_string()).bind(now).execute(repo.pool()).await.unwrap();
    let insert="INSERT INTO video_merge_previews(id,task_id,media_item_id,preview_json,detail_json,selected_artwork,expected_media_json,state,created_at) VALUES(?,?,?,'{}','{}','[]','{}','planned',?)";
    sqlx::query(insert)
        .bind(before.to_string())
        .bind(task.to_string())
        .bind(ids[0].to_string())
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();
    let queue = service.create(create(vec![ids[0]])).await.unwrap();
    // Force the same timestamp to exercise coarse Windows clock resolution.
    sqlx::query("UPDATE review_queues SET created_at=? WHERE id=?")
        .bind(now)
        .bind(queue.id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    let mut input = ReviewQueueCheckpoint {
        queue_id: queue.id,
        media_id: ids[0],
        expected_revision: queue.revision,
        preview_id: before,
    };
    assert!(matches!(
        service.checkpoint(input.clone()).await,
        Err(RepositoryError::Conflict)
    ));
    let after = Uuid::new_v4();
    sqlx::query(insert)
        .bind(after.to_string())
        .bind(task.to_string())
        .bind(ids[0].to_string())
        .bind(now)
        .execute(repo.pool())
        .await
        .unwrap();
    input.preview_id = after;
    let bound = service.checkpoint(input).await.unwrap();
    assert_eq!(bound.items[0].preview_id, Some(after));
    assert!(bound.items[0].checkpoint.is_some());
    repo.pool().close().await;
}
