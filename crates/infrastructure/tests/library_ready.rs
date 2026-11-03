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
    let dir = tempfile::tempdir().unwrap();
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
async fn only_explicit_real_metadata_becomes_consumable_after_scan_confirmation() {
    let (_dir, repo, browse) = imported(&[
        ("Bare.2020.mkv", None),
        ("Named.2021.mkv", Some("NFO title")),
    ])
    .await;
    let tools = browse.tools_list_media(&Default::default()).await.unwrap();
    assert_eq!(tools.items.len(), 2);
    let ready = browse.list_media(&Default::default()).await.unwrap();
    assert_eq!(ready.items.len(), 1);
    assert_eq!(ready.items[0].media.title, "NFO title");
    let bare = tools
        .items
        .iter()
        .find(|i| i.media.title != "NFO title")
        .unwrap();
    assert_eq!(
        browse
            .get_media(&MediaDetailInput {
                media_id: bare.media.id,
                include_adult: false
            })
            .await
            .unwrap_err(),
        RepositoryError::NotFound
    );
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
        "local provider label alone is insufficient evidence"
    );
    assert_eq!(
        browse
            .summary(&Default::default())
            .await
            .unwrap()
            .total_media,
        1
    );
    sqlx::query("DELETE FROM media_files WHERE media_item_id=?")
        .bind(ready.items[0].media.id.to_string())
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
    assert_eq!(titles, ["B", "D", "E"]);
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
        1
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
