use super::*;
use crate::{browse::BrowseService, library::MetadataWriteMode};
use chrono::Utc;
use mediaatlas_application::library::RootInput;
use mediaatlas_domain::{ExternalId, MediaFile, MediaItem, MediaKind, MetadataValue};
use serde_json::json;

#[tokio::test]
async fn bounded_health_review_finds_missing_assets_duplicates_and_offline_files_without_writes() {
    let dir = crate::test_support::tempdir().unwrap();
    let media = dir.path().join("media");
    std::fs::create_dir(&media).unwrap();
    let repo = LibraryRepository::open(&dir.path().join("db.sqlite"))
        .await
        .unwrap();
    let root = BrowseService::new(repo.clone(), dir.path().join("cache"))
        .save_root(&RootInput {
            nfo_export_mode: None,
            metadata_providers: None,
            id: None,
            name: "Fixture".into(),
            path: media.to_str().unwrap().into(),
            kind: Some(MediaKind::Movie),
            enabled: true,
            exclude_patterns: vec![],
        })
        .await
        .unwrap()
        .root;
    let mut ids = Vec::new();
    for n in 1..=3 {
        let id = Uuid::from_u128(n);
        ids.push(id);
        let now = Utc::now();
        repo.save_media(&MediaItem {
            id,
            kind: if n == 3 {
                MediaKind::AdultVideo
            } else {
                MediaKind::Movie
            },
            title: format!("Fixture {n}"),
            original_title: None,
            year: None,
            created_at: now,
            updated_at: now,
        })
        .await
        .unwrap();
        repo.save_external_id(&ExternalId {
            id: Uuid::new_v4(),
            media_item_id: id,
            provider_id: "fixture".into(),
            namespace: "movie".into(),
            external_id: "same".into(),
            is_primary: true,
            created_at: now,
        })
        .await
        .unwrap();
        repo.save_file(&MediaFile {
            id: Uuid::new_v4(),
            media_item_id: id,
            root_id: root.id,
            relative_path: format!("{n}.mkv"),
            size_bytes: 7,
            modified_at: None,
            part: None,
        })
        .await
        .unwrap();
    }
    std::fs::write(media.join("1.mkv"), b"fixture").unwrap();
    repo.save_metadata_value(
        &MetadataValue {
            id: Uuid::new_v4(),
            media_item_id: ids[0],
            field_key: "title".into(),
            locale: None,
            value: json!("confirmed"),
            provider_id: None,
            source_url: None,
            fetched_at: None,
            confidence: None,
            is_user_modified: true,
            is_locked: false,
        },
        MetadataWriteMode::User,
    )
    .await
    .unwrap();
    let service = HealthService::new(repo.clone());
    let first = service
        .check(&HealthQuery {
            cursor: None,
            limit: 1,
        })
        .await
        .unwrap();
    assert_eq!(first.items.len(), 1);
    assert_eq!(first.items[0].media_id, ids[0]);
    assert!(first.next_cursor.is_some());
    assert!(first.items[0].issues.contains(&"missing_poster".into()));
    assert!(first.items[0].issues.contains(&"duplicate_identity".into()));
    assert!(!first.items[0].issues.contains(&"offline_file".into()));
    assert!(!first.items[0]
        .issues
        .contains(&"incomplete_metadata".into()));
    let next = service
        .check(&HealthQuery {
            cursor: first.next_cursor,
            limit: 200,
        })
        .await
        .unwrap();
    assert_eq!(next.items.len(), 1);
    assert_eq!(next.items[0].media_id, ids[1]);
    assert!(next.items[0].issues.contains(&"offline_file".into()));
    assert!(next.next_cursor.is_none());
    assert_eq!(std::fs::read(media.join("1.mkv")).unwrap(), b"fixture");
    assert!(!media.join("2.mkv").exists());
    assert!(service
        .check(&HealthQuery {
            cursor: None,
            limit: 0
        })
        .await
        .is_err());
    assert!(service
        .check(&HealthQuery {
            cursor: Some("invalid".into()),
            limit: 1
        })
        .await
        .is_err());
    // An isolated record stays invisible even when reclassified as ordinary.
    sqlx::query("UPDATE media_items SET adult_isolated=1 WHERE id=?")
        .bind(ids[1].to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    let page = service
        .check(&HealthQuery {
            cursor: None,
            limit: 200,
        })
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert!(!page.items[0].issues.contains(&"duplicate_identity".into()));
}
#[test]
fn file_checks_do_not_follow_symlinks_or_traversal() {
    let dir = crate::test_support::tempdir().unwrap();
    std::fs::write(dir.path().join("real.mkv"), b"fixture").unwrap();
    assert!(regular_file_available(dir.path(), Path::new("real.mkv")));
    assert!(!regular_file_available(
        dir.path(),
        Path::new("../real.mkv")
    ));
    assert!(!regular_file_available(dir.path(), Path::new(".")));
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.path().join("real.mkv"), dir.path().join("link.mkv"))
            .unwrap();
        assert!(!regular_file_available(dir.path(), Path::new("link.mkv")));
    }
}

#[tokio::test]
async fn missing_actual_local_and_approved_cached_artwork_is_reported_without_reading_arbitrary_database_paths(
) {
    use mediaatlas_domain::{Artwork, ArtworkKind};
    let dir = crate::test_support::tempdir().unwrap();
    let media = dir.path().join("media");
    let cache = dir.path().join("approved-cache");
    std::fs::create_dir(&media).unwrap();
    std::fs::create_dir(&cache).unwrap();
    let repo = LibraryRepository::open(&dir.path().join("db.sqlite"))
        .await
        .unwrap();
    let root = BrowseService::new(repo.clone(), dir.path().join("thumbnails"))
        .save_root(&RootInput {
            nfo_export_mode: None,
            metadata_providers: None,
            id: None,
            name: "Fixture".into(),
            path: media.to_str().unwrap().into(),
            kind: Some(MediaKind::Movie),
            enabled: true,
            exclude_patterns: vec![],
        })
        .await
        .unwrap()
        .root;
    let now = Utc::now();
    let id = Uuid::new_v4();
    repo.save_media(&MediaItem {
        id,
        kind: MediaKind::Movie,
        title: "Artwork fixture".into(),
        original_title: None,
        year: None,
        created_at: now,
        updated_at: now,
    })
    .await
    .unwrap();
    std::fs::write(media.join("fixture.mkv"), b"fixture").unwrap();
    repo.save_file(&MediaFile {
        id: Uuid::new_v4(),
        media_item_id: id,
        root_id: root.id,
        relative_path: "fixture.mkv".into(),
        size_bytes: 7,
        modified_at: None,
        part: None,
    })
    .await
    .unwrap();
    let local = Uuid::new_v4();
    let cached = Uuid::new_v4();
    let arbitrary = Uuid::new_v4();
    let unrelated = dir.path().join("unapproved.png");
    std::fs::write(&unrelated, b"unrelated bytes").unwrap();
    for (art_id, path, url) in [
        (
            local,
            None,
            Some(format!("library://{}/poster.png", root.id)),
        ),
        (
            cached,
            Some(cache.join("cached.png").to_str().unwrap().into()),
            Some("https://example.invalid/poster.png".into()),
        ),
        (arbitrary, Some(unrelated.to_str().unwrap().into()), None),
    ] {
        repo.save_artwork(&Artwork {
            id: art_id,
            media_item_id: id,
            kind: ArtworkKind::Poster,
            cache_path: path,
            source_url: url,
            provider_id: None,
            width: None,
            height: None,
            content_hash: None,
            created_at: now,
        })
        .await
        .unwrap();
    }
    let service = HealthService::new(repo.clone()).with_artwork_cache(cache.clone());
    let query = HealthQuery {
        cursor: None,
        limit: 100,
    };
    let page = service.check(&query).await.unwrap();
    assert!(page.items[0]
        .issues
        .contains(&"missing_artwork_asset".into()));
    assert!(page.items[0]
        .issues
        .contains(&"unverified_artwork_asset".into()));
    std::fs::write(media.join("poster.png"), b"local fixture").unwrap();
    std::fs::write(cache.join("cached.png"), b"cache fixture").unwrap();
    sqlx::query("DELETE FROM artworks WHERE id=?")
        .bind(arbitrary.to_string())
        .execute(repo.pool())
        .await
        .unwrap();
    let page = service.check(&query).await.unwrap();
    assert!(!page.items[0]
        .issues
        .contains(&"missing_artwork_asset".into()));
    assert!(!page.items[0]
        .issues
        .contains(&"unverified_artwork_asset".into()));
    assert_eq!(std::fs::read(unrelated).unwrap(), b"unrelated bytes");
    #[cfg(unix)]
    {
        std::fs::remove_file(cache.join("cached.png")).unwrap();
        std::os::unix::fs::symlink(media.join("poster.png"), cache.join("cached.png")).unwrap();
        assert!(service.check(&query).await.unwrap().items[0]
            .issues
            .contains(&"missing_artwork_asset".into()));
    }
}
