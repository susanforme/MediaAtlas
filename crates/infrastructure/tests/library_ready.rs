mod support;

use mediaatlas_application::library::{
    BrowseQuery, ConfirmInput, MediaDetailInput, PreviewInput, RootInput,
};
use mediaatlas_domain::{MediaKind, TaskState};
use mediaatlas_infrastructure::{
    browse::BrowseService,
    library::{LibraryRepository, RepositoryError},
    library_ready::backfill_local_nfo,
    scan_service::ScanService,
};
use std::{path::Path, time::Duration};

async fn imported(
    names: &[(&str, Option<&str>)],
) -> (tempfile::TempDir, LibraryRepository, BrowseService) {
    let dir = support::tempdir().unwrap();
    let media = dir.path().join("media");
    std::fs::create_dir(&media).unwrap();
    for (name, nfo) in names {
        let path = media.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"temporary video bytes").unwrap();
        if let Some(title) = nfo {
            std::fs::write(
                path.with_extension("nfo"),
                format!("<movie><title>{title}</title></movie>"),
            )
            .unwrap();
        }
    }
    let repo = LibraryRepository::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let browse = BrowseService::new(repo.clone(), dir.path().join("thumbs"));
    let root = browse
        .save_root(&RootInput {
            metadata_providers: None,
            id: None,
            name: "Fixtures".into(),
            path: media.to_str().unwrap().into(),
            kind: Some(MediaKind::Movie),
            enabled: true,
            exclude_patterns: vec![],
        })
        .await
        .unwrap();
    let scans = ScanService::new(repo.clone());
    let task = scans.start(root.root.id).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if scans
                .list_tasks(false)
                .await
                .unwrap()
                .iter()
                .any(|t| t.id == task.id && t.state == TaskState::WaitingForUser)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(browse
        .list_media(&BrowseQuery::default())
        .await
        .unwrap()
        .items
        .is_empty());
    let page = scans
        .preview(&PreviewInput {
            task_id: task.id,
            cursor: None,
            limit: 200,
            include_adult: false,
        })
        .await
        .unwrap();
    scans
        .confirm(&ConfirmInput {
            task_id: task.id,
            candidate_ids: page.items.iter().map(|c| c.id).collect(),
        })
        .await
        .unwrap();
    (dir, repo, browse)
}

#[tokio::test]
async fn local_files_are_browsable_before_metadata_enrichment_and_remote_only_remains_hidden() {
    let (_dir, repo, browse) = imported(&[
        ("Bare.2020.mkv", None),
        ("Named.2021.mkv", Some("NFO title")),
    ])
    .await;
    let tools = browse.tools_list_media(&Default::default()).await.unwrap();
    assert_eq!(tools.items.len(), 2);
    let ready = browse.list_media(&Default::default()).await.unwrap();
    assert_eq!(ready.items.len(), 2);
    let enriched = ready
        .items
        .iter()
        .find(|i| i.media.title == "NFO title")
        .unwrap();
    assert!(!enriched.health.contains(&"incomplete_metadata".into()));
    let bare = tools
        .items
        .iter()
        .find(|i| i.media.title != "NFO title")
        .unwrap();
    let pending = browse
        .get_media(&MediaDetailInput {
            media_id: bare.media.id,
            include_adult: false,
        })
        .await
        .unwrap();
    assert!(pending.health.contains(&"incomplete_metadata".into()));
    let pending_page = browse
        .list_media(&BrowseQuery {
            health: Some("incomplete_metadata".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(pending_page.items.len(), 1);
    assert_eq!(pending_page.items[0].media.id, bare.media.id);
    assert!(browse
        .tools_media_detail(bare.media.id, false)
        .await
        .is_ok());
    let local_titles: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM metadata_values WHERE provider_id='local' AND field_key='title'",
    )
    .fetch_one(repo.pool())
    .await
    .unwrap();
    assert_eq!(
        local_titles, 2,
        "both local filename and NFO titles are persisted, but only NFO completes metadata"
    );
    assert_eq!(
        browse
            .summary(&Default::default())
            .await
            .unwrap()
            .total_media,
        2
    );
    sqlx::query("DELETE FROM media_files WHERE media_item_id=?")
        .bind(enriched.media.id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    let remaining = browse.list_media(&Default::default()).await.unwrap();
    assert_eq!(remaining.items.len(), 1);
    assert_eq!(remaining.items[0].media.id, bare.media.id);
    assert_eq!(
        browse
            .get_media(&MediaDetailInput {
                media_id: enriched.media.id,
                include_adult: false,
            })
            .await
            .unwrap_err(),
        RepositoryError::NotFound,
        "metadata-only graph never appears as local resource"
    );
}

#[tokio::test]
async fn readiness_filters_before_global_pagination_and_tools_cursor_cannot_cross_scope() {
    let (_dir, _repo, browse) = imported(&[
        ("A.mkv", None),
        ("B.mkv", Some("B")),
        ("C.mkv", None),
        ("D.mkv", Some("D")),
        ("E.mkv", Some("E")),
    ])
    .await;
    let mut query = BrowseQuery {
        limit: 1,
        sort: "title_asc".into(),
        ..Default::default()
    };
    let mut titles = vec![];
    loop {
        let page = browse.list_media(&query).await.unwrap();
        titles.extend(page.items.into_iter().map(|i| i.media.title));
        if page.next_cursor.is_none() {
            break;
        }
        query.cursor = page.next_cursor;
    }
    assert_eq!(titles, ["A", "B", "C", "D", "E"]);
    let tools = browse
        .tools_list_media(&BrowseQuery {
            limit: 1,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(
        browse
            .list_media(&BrowseQuery {
                cursor: tools.next_cursor,
                ..Default::default()
            })
            .await
            .unwrap_err(),
        RepositoryError::InvalidInput
    );
}

#[tokio::test]
async fn legacy_nfo_backfill_requires_valid_existing_imported_metadata_and_current_local_binding() {
    let (dir, repo, browse) =
        imported(&[("One.mkv", Some("Confirmed NFO")), ("Bare.mkv", None)]).await;
    sqlx::query("DELETE FROM local_metadata_provenance")
        .execute(repo.pool())
        .await
        .unwrap();
    let nfo = dir.path().join("media/One.nfo");
    std::fs::write(
        &nfo,
        "<!DOCTYPE movie><movie><title>Confirmed NFO</title></movie>",
    )
    .unwrap();
    assert_eq!(backfill_local_nfo(&repo).await.unwrap(), 0);
    std::fs::write(&nfo, "<movie><title>New unconfirmed title</title></movie>").unwrap();
    assert_eq!(backfill_local_nfo(&repo).await.unwrap(), 0);
    std::fs::write(&nfo, "<movie><title>Confirmed NFO</title></movie>").unwrap();
    assert_eq!(backfill_local_nfo(&repo).await.unwrap(), 1);
    assert_eq!(backfill_local_nfo(&repo).await.unwrap(), 0);
    assert_eq!(
        browse
            .list_media(&Default::default())
            .await
            .unwrap()
            .items
            .len(),
        2
    );
    sqlx::query("UPDATE library_roots SET enabled=0")
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(browse
        .list_media(&Default::default())
        .await
        .unwrap()
        .items
        .is_empty());
    assert!(Path::new(&nfo).exists());
}

#[cfg(unix)]
#[tokio::test]
async fn legacy_backfill_rejects_final_or_ancestor_symlink() {
    let (dir, repo, _browse) = imported(&[("One.mkv", Some("Confirmed NFO"))]).await;
    sqlx::query("DELETE FROM local_metadata_provenance")
        .execute(repo.pool())
        .await
        .unwrap();
    let nfo = dir.path().join("media/One.nfo");
    let outside = dir.path().join("outside.nfo");
    std::fs::rename(&nfo, &outside).unwrap();
    std::os::unix::fs::symlink(&outside, &nfo).unwrap();
    assert_eq!(backfill_local_nfo(&repo).await.unwrap(), 0);
    std::fs::remove_file(&nfo).unwrap();
    std::fs::rename(&outside, &nfo).unwrap();
    let moved = dir.path().join("moved");
    std::fs::rename(dir.path().join("media"), &moved).unwrap();
    std::os::unix::fs::symlink(&moved, dir.path().join("media")).unwrap();
    assert_eq!(backfill_local_nfo(&repo).await.unwrap(), 0);
}

#[tokio::test]
async fn bare_rescan_preserves_confirmed_local_metadata_when_original_nfo_is_removed() {
    let (dir, repo, browse) = imported(&[("One.mkv", Some("Confirmed NFO"))]).await;
    let root = repo.list_roots().await.unwrap().remove(0);
    std::fs::remove_file(dir.path().join("media/One.nfo")).unwrap();
    let scans = ScanService::new(repo.clone());
    let task = scans.start(root.id).await.unwrap();
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if scans
                .list_tasks(false)
                .await
                .unwrap()
                .iter()
                .any(|t| t.id == task.id && t.state == TaskState::WaitingForUser)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    let preview = scans
        .preview(&PreviewInput {
            task_id: task.id,
            cursor: None,
            limit: 100,
            include_adult: false,
        })
        .await
        .unwrap();
    scans
        .confirm(&ConfirmInput {
            task_id: task.id,
            candidate_ids: preview.items.iter().map(|i| i.id).collect(),
        })
        .await
        .unwrap();
    let ready = browse.list_media(&Default::default()).await.unwrap();
    assert_eq!(ready.items.len(), 1);
    assert_eq!(ready.items[0].media.title, "Confirmed NFO");
    assert_eq!(
        browse
            .get_media(&MediaDetailInput {
                media_id: ready.items[0].media.id,
                include_adult: false
            })
            .await
            .unwrap()
            .metadata_values
            .iter()
            .find(|v| v.field_key == "title")
            .unwrap()
            .value,
        serde_json::json!("Confirmed NFO")
    );
}

#[tokio::test]
async fn adult_backfill_requires_explicit_live_grant_and_startup_ordinary_backfill_skips_it() {
    use mediaatlas_infrastructure::{
        adult_privacy::{AdultPrivacyService, AdultSettingsInput},
        library_ready::backfill_adult_local_nfo,
        providers::ProviderService,
    };
    let (_dir, repo, _browse) = imported(&[("One.mkv", Some("Confirmed NFO"))]).await;
    sqlx::query("DELETE FROM local_metadata_provenance")
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE media_items SET kind='adult_video',adult_isolated=1")
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE library_roots SET kind='adult_video'")
        .execute(repo.pool())
        .await
        .unwrap();
    assert_eq!(backfill_local_nfo(&repo).await.unwrap(), 0);
    let providers = ProviderService::new(repo.clone()).await.unwrap();
    let privacy = AdultPrivacyService::new(repo.clone(), providers);
    privacy
        .save(AdultSettingsInput {
            entry_visible: true,
            covers_visible: false,
            remember_recent: false,
        })
        .await
        .unwrap();
    privacy.unlock().await.unwrap();
    let grant = privacy.access().await.unwrap();
    let unlocked = BrowseService::new(repo.clone(), _dir.path().join("adult-thumbnails"))
        .with_adult_access(grant.clone(), false);
    let pending = unlocked
        .list_media(&BrowseQuery {
            include_adult: true,
            kind: Some(MediaKind::AdultVideo),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(
        pending.items.len(),
        1,
        "unlocked local adult files are available before metadata enrichment"
    );
    assert!(pending.items[0]
        .health
        .contains(&"incomplete_metadata".into()));
    assert!(_browse
        .list_media(&Default::default())
        .await
        .unwrap()
        .items
        .is_empty());
    assert_eq!(
        backfill_adult_local_nfo(&repo, grant.clone())
            .await
            .unwrap(),
        1
    );
    sqlx::query("DELETE FROM local_metadata_provenance")
        .execute(repo.pool())
        .await
        .unwrap();
    privacy.lock().await.unwrap();
    assert!(backfill_adult_local_nfo(&repo, grant).await.is_err());
    assert_eq!(backfill_local_nfo(&repo).await.unwrap(), 0);
}

#[tokio::test]
async fn persisted_refresh_outcomes_are_visible_and_manual_correction_clears_stale_failure() {
    let (_dir, repo, browse) = imported(&[("Pending.2020.mkv", None)]).await;
    let card = browse
        .list_media(&Default::default())
        .await
        .unwrap()
        .items
        .remove(0);
    let task: String = sqlx::query_scalar("SELECT task_id FROM scan_sessions LIMIT 1")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    for (state, reason, expected) in [
        ("pending", None, Some("metadata_pending")),
        (
            "needs_review",
            Some("provider_unconfigured"),
            Some("metadata_unconfigured"),
        ),
        (
            "needs_review",
            Some("ambiguous_match"),
            Some("metadata_needs_review"),
        ),
        ("failed", Some("network_error"), Some("metadata_failed")),
        ("skipped", None, None),
        ("matched", None, None),
    ] {
        sqlx::query("INSERT INTO library_refresh_items(task_id,media_item_id,state,reason,updated_at) VALUES(?,?,?,?,1) ON CONFLICT(task_id,media_item_id) DO UPDATE SET state=excluded.state,reason=excluded.reason")
            .bind(&task).bind(card.media.id.to_string()).bind(state).bind(reason)
            .execute(repo.pool()).await.unwrap();
        let current = browse
            .list_media(&Default::default())
            .await
            .unwrap()
            .items
            .remove(0);
        let statuses: Vec<_> = current
            .health
            .iter()
            .filter(|s| s.starts_with("metadata_"))
            .map(String::as_str)
            .collect();
        assert_eq!(statuses, expected.into_iter().collect::<Vec<_>>());
        if let Some(status) = expected {
            let filtered = browse
                .list_media(&BrowseQuery {
                    health: Some(status.into()),
                    ..Default::default()
                })
                .await
                .unwrap();
            assert_eq!(filtered.items.len(), 1);
            assert_eq!(filtered.items[0].media.id, card.media.id);
        }
        assert!(
            current.health.contains(&"incomplete_metadata".into()),
            "task outcome alone is not metadata evidence"
        );
    }
    sqlx::query("UPDATE library_refresh_items SET state='failed'")
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE metadata_values SET is_user_modified=1 WHERE media_item_id=?")
        .bind(card.media.id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    let corrected = browse
        .get_media(&MediaDetailInput {
            media_id: card.media.id,
            include_adult: false,
        })
        .await
        .unwrap();
    assert!(!corrected.health.contains(&"metadata_failed".into()));
    assert!(!corrected.health.contains(&"incomplete_metadata".into()));
    sqlx::query("UPDATE library_refresh_items SET reason='artwork_failed'")
        .execute(repo.pool())
        .await
        .unwrap();
    let art_failed = browse
        .get_media(&MediaDetailInput {
            media_id: card.media.id,
            include_adult: false,
        })
        .await
        .unwrap();
    assert!(
        art_failed.health.contains(&"metadata_failed".into()),
        "missing artwork must remain retryable even when metadata was saved"
    );
    assert_eq!(
        browse
            .list_media(&BrowseQuery {
                health: Some("metadata_failed".into()),
                ..Default::default()
            })
            .await
            .unwrap()
            .items
            .len(),
        1
    );
}

#[tokio::test]
async fn unmatched_local_media_still_respects_root_and_adult_visibility() {
    let (_dir, repo, browse) = imported(&[("Pending.2020.mkv", None)]).await;
    let id = browse.list_media(&Default::default()).await.unwrap().items[0]
        .media
        .id;
    sqlx::query("UPDATE media_items SET adult_isolated=1 WHERE id=?")
        .bind(id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(browse
        .list_media(&Default::default())
        .await
        .unwrap()
        .items
        .is_empty());
    sqlx::query("UPDATE media_items SET adult_isolated=0 WHERE id=?")
        .bind(id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE library_roots SET kind='adult_video'")
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(browse
        .list_media(&Default::default())
        .await
        .unwrap()
        .items
        .is_empty());
    sqlx::query("UPDATE library_roots SET kind='movie',enabled=0")
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(browse
        .list_media(&Default::default())
        .await
        .unwrap()
        .items
        .is_empty());
}

#[tokio::test]
async fn typed_series_are_visible_from_local_episodes_and_share_refresh_state() {
    let (_dir, repo, browse) = imported(&[("Show.2020.mkv", None)]).await;
    let episode = browse
        .list_media(&Default::default())
        .await
        .unwrap()
        .items
        .remove(0)
        .media;
    sqlx::query("UPDATE media_items SET kind='tv_episode' WHERE id=?")
        .bind(episode.id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    sqlx::query("UPDATE library_roots SET kind='tv_series'")
        .execute(repo.pool())
        .await
        .unwrap();
    let mut series = episode.clone();
    series.id = uuid::Uuid::new_v4();
    series.kind = MediaKind::TvSeries;
    series.title = "Show".into();
    repo.save_media(&series).await.unwrap();
    sqlx::query(
        "INSERT INTO media_relations(parent_id,child_id,relation_type) VALUES(?,?,'episode')",
    )
    .bind(series.id.to_string())
    .bind(episode.id.to_string())
    .execute(repo.pool())
    .await
    .unwrap();
    let task: String = sqlx::query_scalar("SELECT task_id FROM scan_sessions LIMIT 1")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    sqlx::query("INSERT INTO library_refresh_items(task_id,media_item_id,state,reason,updated_at) VALUES(?,?,'pending',NULL,1)")
        .bind(&task).bind(series.id.to_string()).execute(repo.pool()).await.unwrap();
    let page = browse.list_media(&Default::default()).await.unwrap();
    assert_eq!(page.items.len(), 2);
    assert!(page
        .items
        .iter()
        .all(|i| i.health.contains(&"metadata_pending".into())));
    let detail = browse
        .get_media(&MediaDetailInput {
            media_id: series.id,
            include_adult: false,
        })
        .await
        .unwrap();
    assert!(detail.files.iter().any(|f| f.media_item_id == episode.id));
    sqlx::query("UPDATE media_items SET adult_isolated=1 WHERE id=?")
        .bind(series.id.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    assert!(
        browse
            .list_media(&Default::default())
            .await
            .unwrap()
            .items
            .is_empty(),
        "an isolated ancestor must not leak a local descendant"
    );
}

#[tokio::test]
async fn newest_refresh_generation_wins_even_when_an_old_worker_finishes_later() {
    let (_dir, repo, browse) = imported(&[("Pending.2020.mkv", None)]).await;
    let media = browse
        .list_media(&Default::default())
        .await
        .unwrap()
        .items
        .remove(0)
        .media;
    let old_task: String = sqlx::query_scalar("SELECT task_id FROM scan_sessions LIMIT 1")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    let new_task = uuid::Uuid::new_v4().to_string();
    // Deliberately identical creation timestamps exercise the insertion-order tie.
    sqlx::query("INSERT INTO scrape_tasks(id,kind,state,input_snapshot,created_at,updated_at) SELECT ?,kind,'waiting_for_user',input_snapshot,created_at,updated_at FROM scrape_tasks WHERE id=?")
        .bind(&new_task).bind(&old_task).execute(repo.pool()).await.unwrap();
    sqlx::query("INSERT INTO scan_sessions(task_id,root_id,root_snapshot_json) SELECT ?,root_id,root_snapshot_json FROM scan_sessions WHERE task_id=?")
        .bind(&new_task).bind(&old_task).execute(repo.pool()).await.unwrap();
    for (task, state, reason, updated) in [
        (&old_task, "needs_review", "cancelled", 200),
        (&new_task, "failed", "network_error", 100),
    ] {
        sqlx::query("INSERT INTO library_refresh_items(task_id,media_item_id,state,reason,updated_at) VALUES(?,?,?,?,?)")
            .bind(task).bind(media.id.to_string()).bind(state).bind(reason).bind(updated)
            .execute(repo.pool()).await.unwrap();
    }
    let current = browse
        .get_media(&MediaDetailInput {
            media_id: media.id,
            include_adult: false,
        })
        .await
        .unwrap();
    assert!(current.health.contains(&"metadata_failed".into()));
    assert!(!current.health.contains(&"metadata_needs_review".into()));
    assert_eq!(
        browse
            .list_media(&BrowseQuery {
                health: Some("metadata_failed".into()),
                ..Default::default()
            })
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    assert!(browse
        .list_media(&BrowseQuery {
            health: Some("metadata_needs_review".into()),
            ..Default::default()
        })
        .await
        .unwrap()
        .items
        .is_empty());
    sqlx::query("UPDATE library_refresh_items SET state='skipped',reason='existing_metadata' WHERE task_id=?")
        .bind(&new_task).execute(repo.pool()).await.unwrap();
    let completed = browse
        .get_media(&MediaDetailInput {
            media_id: media.id,
            include_adult: false,
        })
        .await
        .unwrap();
    assert!(
        !completed.health.iter().any(|s| s.starts_with("metadata_")),
        "terminal newer refresh must suppress all stale older status"
    );
}
