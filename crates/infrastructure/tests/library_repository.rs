use chrono::{DateTime, Utc};
use mediaatlas_domain::{
    Artwork, ArtworkKind, ExternalId, LibraryRoot, MediaFile, MediaItem, MediaKind, MetadataValue,
    PathStyle,
};
use mediaatlas_infrastructure::library::{
    LibraryRepository, ListMediaQuery, MetadataWriteMode, RepositoryError,
};
use serde_json::json;
use tempfile::TempDir;
use uuid::Uuid;

fn timestamp() -> DateTime<Utc> {
    DateTime::from_timestamp_millis(1_700_000_000_123).unwrap()
}

async fn database() -> (TempDir, LibraryRepository) {
    let dir = tempfile::tempdir().unwrap();
    let repo = LibraryRepository::open(&dir.path().join("library.sqlite"))
        .await
        .unwrap();
    (dir, repo)
}

fn root(path: &str, path_style: PathStyle) -> LibraryRoot {
    LibraryRoot {
        id: Uuid::new_v4(),
        name: "Fixture root".into(),
        path: path.into(),
        path_style,
        kind: None,
        enabled: true,
        created_at: timestamp(),
        updated_at: timestamp(),
    }
}

fn media(id: u128, kind: MediaKind) -> MediaItem {
    MediaItem {
        id: Uuid::from_u128(id),
        kind,
        title: format!("Fixture {id}"),
        original_title: None,
        year: Some(2024),
        created_at: timestamp(),
        updated_at: timestamp(),
    }
}

fn file(item: &MediaItem, root: &LibraryRoot, path: &str) -> MediaFile {
    MediaFile {
        id: Uuid::new_v4(),
        media_item_id: item.id,
        root_id: root.id,
        relative_path: path.into(),
        size_bytes: 12,
        modified_at: Some(timestamp()),
        part: Some(1),
    }
}

fn field(item: &MediaItem, locale: Option<&str>) -> MetadataValue {
    MetadataValue {
        id: Uuid::new_v4(),
        media_item_id: item.id,
        field_key: "title".into(),
        locale: locale.map(str::to_owned),
        value: json!("Fixture title"),
        provider_id: Some("fixture-provider".into()),
        source_url: Some("https://example.test/item".into()),
        fetched_at: Some(timestamp()),
        confidence: Some(0.8),
        is_user_modified: false,
        is_locked: false,
    }
}

fn external(item: &MediaItem, value: &str) -> ExternalId {
    ExternalId {
        id: Uuid::new_v4(),
        media_item_id: item.id,
        provider_id: "fixture".into(),
        namespace: "movie".into(),
        external_id: value.into(),
        is_primary: false,
        created_at: timestamp(),
    }
}

fn artwork(item: &MediaItem) -> Artwork {
    Artwork {
        id: Uuid::new_v4(),
        media_item_id: item.id,
        kind: ArtworkKind::Poster,
        cache_path: Some("posters/fixture.jpg".into()),
        source_url: Some("https://example.test/poster.jpg".into()),
        provider_id: Some("fixture".into()),
        width: Some(600),
        height: Some(900),
        content_hash: Some("fixture-hash".into()),
        created_at: timestamp(),
    }
}

#[tokio::test]
async fn migrations_reopen_and_connection_pragmas_are_enforced() {
    let (dir, repo) = database().await;
    let item = media(1, MediaKind::Movie);
    repo.save_media(&item).await.unwrap();
    let mut connections = Vec::new();
    for _ in 0..4 {
        let mut connection = repo.pool().acquire().await.unwrap();
        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        let timeout: i64 = sqlx::query_scalar("PRAGMA busy_timeout")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        assert_eq!(foreign_keys, 1);
        assert_eq!(timeout, 5000);
        assert_eq!(journal, "wal");
        connections.push(connection);
    }
    drop(connections);
    repo.pool().close().await;
    let reopened = LibraryRepository::open(&dir.path().join("library.sqlite"))
        .await
        .unwrap();
    assert_eq!(reopened.get_media(item.id).await.unwrap(), Some(item));
    let versions: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations WHERE success=1")
        .fetch_one(reopened.pool())
        .await
        .unwrap();
    assert!(versions >= 1);
    sqlx::query("UPDATE _sqlx_migrations SET checksum=x'00' WHERE version=1")
        .execute(reopened.pool())
        .await
        .unwrap();
    reopened.pool().close().await;
    assert!(matches!(
        LibraryRepository::open(&dir.path().join("library.sqlite")).await,
        Err(RepositoryError::Migration)
    ));
}

#[tokio::test]
async fn all_media_kinds_and_multiple_namespaced_external_ids_persist() {
    let (_dir, repo) = database().await;
    for (i, kind) in [
        MediaKind::Movie,
        MediaKind::TvSeries,
        MediaKind::TvSeason,
        MediaKind::TvEpisode,
        MediaKind::Anime,
        MediaKind::AdultVideo,
        MediaKind::MusicArtist,
        MediaKind::MusicReleaseGroup,
        MediaKind::MusicRelease,
        MediaKind::MusicTrack,
        MediaKind::MusicRecording,
        MediaKind::MusicVideo,
        MediaKind::Concert,
    ]
    .into_iter()
    .enumerate()
    {
        let item = media(i as u128 + 1, kind);
        repo.save_media(&item).await.unwrap();
        assert_eq!(repo.get_media(item.id).await.unwrap(), Some(item));
    }
    let first = media(1, MediaKind::Movie);
    let other = media(2, MediaKind::TvSeries);
    let mut id = external(&first, "100");
    id.is_primary = true;
    repo.save_external_id(&id).await.unwrap();
    let mut other_namespace = external(&first, "200");
    other_namespace.namespace = "imdb".into();
    repo.save_external_id(&other_namespace).await.unwrap();
    let shared = external(&other, "100");
    repo.save_external_id(&shared).await.unwrap();
    let duplicate = ExternalId {
        id: Uuid::new_v4(),
        ..id.clone()
    };
    assert_eq!(
        repo.save_external_id(&duplicate).await,
        Err(RepositoryError::Conflict)
    );
    let mut competing_primary = external(&first, "101");
    competing_primary.is_primary = true;
    assert_eq!(
        repo.save_external_id(&competing_primary).await,
        Err(RepositoryError::Conflict)
    );
    assert_eq!(repo.list_external_ids(first.id).await.unwrap().len(), 2);
}

#[tokio::test]
async fn windows_identity_is_case_insensitive_while_posix_keeps_case() {
    let (_dir, repo) = database().await;
    let windows = root("C:\\Fixture", PathStyle::Windows);
    let posix = root("/fixture", PathStyle::Posix);
    let unc = root("\\\\server\\share\\Fixture", PathStyle::Windows);
    for r in [&windows, &posix, &unc] {
        repo.save_root(r).await.unwrap();
    }
    let duplicate_root = root("c:/fixture/", PathStyle::Windows);
    assert_eq!(
        repo.save_root(&duplicate_root).await,
        Err(RepositoryError::Conflict)
    );
    let duplicate_unc = root("//SERVER/SHARE/fixture", PathStyle::Windows);
    assert_eq!(
        repo.save_root(&duplicate_unc).await,
        Err(RepositoryError::Conflict)
    );
    let item = media(1, MediaKind::Movie);
    repo.save_media(&item).await.unwrap();
    repo.save_file(&file(&item, &windows, "Folder\\Film.MKV"))
        .await
        .unwrap();
    assert_eq!(
        repo.save_file(&file(&item, &windows, "folder/film.mkv"))
            .await,
        Err(RepositoryError::Conflict)
    );
    repo.save_file(&file(&item, &posix, "Film.mkv"))
        .await
        .unwrap();
    repo.save_file(&file(&item, &posix, "film.mkv"))
        .await
        .unwrap();
    for path in ["../escape.mkv", "/absolute.mkv", "C:\\absolute.mkv"] {
        assert_eq!(
            repo.save_file(&file(&item, &windows, path)).await,
            Err(RepositoryError::InvalidInput)
        );
    }
    let mut changed_style = windows.clone();
    changed_style.path = "/fixture-new".into();
    changed_style.path_style = PathStyle::Posix;
    assert_eq!(
        repo.save_root(&changed_style).await,
        Err(RepositoryError::Conflict)
    );
    assert_eq!(repo.list_files(item.id).await.unwrap().len(), 3);
}

#[tokio::test]
async fn keyset_pages_tied_timestamps_without_duplicates_and_combines_filters() {
    let (_dir, repo) = database().await;
    let first_root = root("/fixture-a", PathStyle::Posix);
    let second_root = root("/fixture-b", PathStyle::Posix);
    repo.save_root(&first_root).await.unwrap();
    repo.save_root(&second_root).await.unwrap();
    for i in 1..=7 {
        let mut item = media(
            i,
            if i == 7 {
                MediaKind::AdultVideo
            } else {
                MediaKind::Movie
            },
        );
        // Millisecond storage deliberately normalizes finer precision.
        item.updated_at += chrono::Duration::nanoseconds(789);
        repo.save_media(&item).await.unwrap();
        repo.save_file(&file(
            &item,
            if i == 6 { &second_root } else { &first_root },
            &format!("{i}.mkv"),
        ))
        .await
        .unwrap();
        if i == 1 {
            repo.save_file(&file(&item, &first_root, "1.part2.mkv"))
                .await
                .unwrap();
        }
    }
    let mut query = ListMediaQuery {
        kind: Some(MediaKind::Movie),
        root_id: Some(first_root.id),
        limit: 2,
        ..Default::default()
    };
    let mut seen = Vec::new();
    loop {
        let page = repo.list_media(&query).await.unwrap();
        seen.extend(page.items.iter().map(|m| m.id));
        query.cursor = page.next_cursor;
        if query.cursor.is_none() {
            break;
        }
        assert_eq!(query.cursor.as_ref().unwrap().updated_at, timestamp());
    }
    assert_eq!(seen, (1..=5).map(Uuid::from_u128).collect::<Vec<_>>());
    for limit in [0, 201] {
        assert!(matches!(
            repo.list_media(&ListMediaQuery {
                limit,
                ..Default::default()
            })
            .await,
            Err(RepositoryError::InvalidInput)
        ));
    }
}

#[tokio::test]
async fn search_treats_wildcards_and_sql_as_literal_input() {
    let (_dir, repo) = database().await;
    let mut exact = media(1, MediaKind::Movie);
    exact.title = "100%_Fixture".into();
    repo.save_media(&exact).await.unwrap();
    repo.save_media(&media(2, MediaKind::Movie)).await.unwrap();
    let page = repo
        .list_media(&ListMediaQuery {
            query: Some("%_".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, exact.id);
    let page = repo
        .list_media(&ListMediaQuery {
            query: Some("' OR 1=1 --".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(page.items.is_empty());
    assert_eq!(
        repo.list_media(&ListMediaQuery::default())
            .await
            .unwrap()
            .items
            .len(),
        2
    );
}

#[tokio::test]
async fn provider_cannot_overwrite_user_or_locked_fields_or_rebind_ownership() {
    let (dir, repo) = database().await;
    let item = media(1, MediaKind::Movie);
    let other = media(2, MediaKind::Movie);
    repo.save_media(&item).await.unwrap();
    repo.save_media(&other).await.unwrap();
    let mut value = field(&item, Some("zh-CN"));
    value.is_locked = true;
    repo.save_metadata_value(&value, MetadataWriteMode::User)
        .await
        .unwrap();
    value.is_locked = false;
    value.value = json!("Provider replacement");
    assert_eq!(
        repo.save_metadata_value(&value, MetadataWriteMode::Provider)
            .await,
        Err(RepositoryError::ProtectedMetadata)
    );
    // User may explicitly unlock, but a manual value remains protected.
    repo.save_metadata_value(&value, MetadataWriteMode::User)
        .await
        .unwrap();
    assert_eq!(
        repo.save_metadata_value(&value, MetadataWriteMode::Provider)
            .await,
        Err(RepositoryError::ProtectedMetadata)
    );
    let mut english = field(&item, Some("en"));
    repo.save_metadata_value(&english, MetadataWriteMode::Provider)
        .await
        .unwrap();
    english.value = json!("Provider refreshed English");
    repo.save_metadata_value(&english, MetadataWriteMode::Provider)
        .await
        .unwrap();
    let mut rebind = english.clone();
    rebind.media_item_id = other.id;
    assert_eq!(
        repo.save_metadata_value(&rebind, MetadataWriteMode::User)
            .await,
        Err(RepositoryError::Conflict)
    );
    let mut rename = english.clone();
    rename.field_key = "overview".into();
    assert_eq!(
        repo.save_metadata_value(&rename, MetadataWriteMode::Provider)
            .await,
        Err(RepositoryError::Conflict)
    );
    let mut relocalize = english.clone();
    relocalize.locale = None;
    assert_eq!(
        repo.save_metadata_value(&relocalize, MetadataWriteMode::User)
            .await,
        Err(RepositoryError::Conflict)
    );
    let unset = field(&item, None);
    repo.save_metadata_value(&unset, MetadataWriteMode::Provider)
        .await
        .unwrap();
    let duplicate = MetadataValue {
        id: Uuid::new_v4(),
        ..unset
    };
    assert_eq!(
        repo.save_metadata_value(&duplicate, MetadataWriteMode::Provider)
            .await,
        Err(RepositoryError::Conflict)
    );
    let mut invalid = field(&item, Some("de"));
    invalid.confidence = Some(f64::NAN);
    assert_eq!(
        repo.save_metadata_value(&invalid, MetadataWriteMode::Provider)
            .await,
        Err(RepositoryError::InvalidInput)
    );
    let values = repo.list_metadata_values(item.id).await.unwrap();
    assert_eq!(values.len(), 3);
    let manual = values
        .iter()
        .find(|v| v.locale.as_deref() == Some("zh-cn"))
        .unwrap();
    assert!(manual.is_user_modified);
    assert!(!manual.is_locked);
    repo.pool().close().await;
    let reopened = LibraryRepository::open(&dir.path().join("library.sqlite"))
        .await
        .unwrap();
    assert_eq!(
        reopened.list_metadata_values(item.id).await.unwrap(),
        values
    );
}

#[tokio::test]
async fn equivalent_locales_cannot_bypass_protection_and_ambiguous_keys_are_rejected() {
    let (_dir, repo) = database().await;
    let item = media(1, MediaKind::Movie);
    repo.save_media(&item).await.unwrap();
    let mut manual = field(&item, Some("en-US"));
    manual.is_locked = true;
    manual.value = json!("User-selected regional title");
    repo.save_metadata_value(&manual, MetadataWriteMode::User)
        .await
        .unwrap();

    let provider_variant = field(&item, Some("en-us"));
    assert_eq!(
        repo.save_metadata_value(&provider_variant, MetadataWriteMode::Provider)
            .await,
        Err(RepositoryError::ProtectedMetadata)
    );
    for locale in [" en-US ", "en-US ", " en-US", "en--US", "en_US", "en-美"] {
        assert_eq!(
            repo.save_metadata_value(&field(&item, Some(locale)), MetadataWriteMode::Provider)
                .await,
            Err(RepositoryError::InvalidInput)
        );
    }
    for key in [" title", "title ", "title\0"] {
        let mut invalid = field(&item, Some("en-US"));
        invalid.field_key = key.into();
        assert_eq!(
            repo.save_metadata_value(&invalid, MetadataWriteMode::Provider)
                .await,
            Err(RepositoryError::InvalidInput)
        );
    }

    let british = field(&item, Some("en-GB"));
    repo.save_metadata_value(&british, MetadataWriteMode::Provider)
        .await
        .unwrap();
    let values = repo.list_metadata_values(item.id).await.unwrap();
    assert_eq!(values.len(), 2);
    let protected = values
        .iter()
        .find(|value| value.locale.as_deref() == Some("en-us"))
        .unwrap();
    assert_eq!(protected.value, manual.value);
    assert!(protected.is_locked && protected.is_user_modified);
    assert!(values
        .iter()
        .any(|value| value.locale.as_deref() == Some("en-gb")));
}

#[tokio::test]
async fn child_identity_cannot_be_reassigned_and_parent_updates_do_not_replace_rows() {
    let (_dir, repo) = database().await;
    let mut r = root("/fixture-a", PathStyle::Posix);
    repo.save_root(&r).await.unwrap();
    let other_root = root("/fixture-b", PathStyle::Posix);
    repo.save_root(&other_root).await.unwrap();
    let mut item = media(1, MediaKind::Movie);
    let other = media(2, MediaKind::Movie);
    repo.save_media(&item).await.unwrap();
    repo.save_media(&other).await.unwrap();
    let f = file(&item, &r, "fixture.mkv");
    repo.save_file(&f).await.unwrap();
    let mut reassigned = f.clone();
    reassigned.media_item_id = other.id;
    assert_eq!(
        repo.save_file(&reassigned).await,
        Err(RepositoryError::Conflict)
    );
    reassigned = f.clone();
    reassigned.root_id = other_root.id;
    assert_eq!(
        repo.save_file(&reassigned).await,
        Err(RepositoryError::Conflict)
    );
    let art = artwork(&item);
    repo.save_artwork(&art).await.unwrap();
    assert_eq!(
        repo.save_artwork(&Artwork {
            media_item_id: other.id,
            ..art.clone()
        })
        .await,
        Err(RepositoryError::Conflict)
    );
    let ext = external(&item, "1");
    repo.save_external_id(&ext).await.unwrap();
    assert_eq!(
        repo.save_external_id(&ExternalId {
            media_item_id: other.id,
            ..ext
        })
        .await,
        Err(RepositoryError::Conflict)
    );
    item.title = "Updated fixture".into();
    r.name = "Updated root".into();
    repo.save_media(&item).await.unwrap();
    repo.save_root(&r).await.unwrap();
    assert_eq!(repo.list_files(item.id).await.unwrap(), vec![f]);
    assert_eq!(repo.list_artworks(item.id).await.unwrap(), vec![art]);
    assert_eq!(repo.get_root(r.id).await.unwrap(), Some(r));
}

#[tokio::test]
async fn cascades_remove_only_indexes_and_never_delete_source_or_artwork_files() {
    let (dir, repo) = database().await;
    let source = dir.path().join("fixture.mkv");
    let poster = dir.path().join("poster.jpg");
    std::fs::write(&source, b"Fixture media").unwrap();
    std::fs::write(&poster, b"Fixture poster").unwrap();
    let r = root(
        dir.path().to_str().unwrap(),
        if cfg!(windows) {
            PathStyle::Windows
        } else {
            PathStyle::Posix
        },
    );
    let item = media(1, MediaKind::Movie);
    repo.save_root(&r).await.unwrap();
    repo.save_media(&item).await.unwrap();
    repo.save_file(&file(&item, &r, "fixture.mkv"))
        .await
        .unwrap();
    repo.save_artwork(&artwork(&item)).await.unwrap();
    repo.save_external_id(&external(&item, "1")).await.unwrap();
    repo.save_metadata_value(&field(&item, None), MetadataWriteMode::Provider)
        .await
        .unwrap();
    assert!(repo.remove_media(item.id).await.unwrap());
    assert!(!repo.remove_media(item.id).await.unwrap());
    assert!(repo.list_files(item.id).await.unwrap().is_empty());
    assert!(repo.list_artworks(item.id).await.unwrap().is_empty());
    assert!(repo.list_external_ids(item.id).await.unwrap().is_empty());
    assert!(repo.list_metadata_values(item.id).await.unwrap().is_empty());
    assert_eq!(std::fs::read(&source).unwrap(), b"Fixture media");
    assert_eq!(std::fs::read(&poster).unwrap(), b"Fixture poster");
    repo.save_media(&item).await.unwrap();
    repo.save_file(&file(&item, &r, "fixture.mkv"))
        .await
        .unwrap();
    assert!(repo.remove_root(r.id).await.unwrap());
    assert!(repo.get_media(item.id).await.unwrap().is_some());
    assert!(repo.list_files(item.id).await.unwrap().is_empty());
    assert!(source.exists());
}

#[tokio::test]
async fn unknown_parent_and_oversized_file_fail_with_safe_errors() {
    let (_dir, repo) = database().await;
    let r = root("/fixture", PathStyle::Posix);
    repo.save_root(&r).await.unwrap();
    let item = media(1, MediaKind::Movie);
    assert_eq!(
        repo.save_file(&file(&item, &r, "fixture.mkv")).await,
        Err(RepositoryError::Conflict)
    );
    repo.save_media(&item).await.unwrap();
    let mut too_large = file(&item, &r, "oversized.mkv");
    too_large.size_bytes = u64::MAX;
    assert_eq!(
        repo.save_file(&too_large).await,
        Err(RepositoryError::InvalidInput)
    );
    let text = RepositoryError::Database.to_string();
    assert!(!text.contains("sqlite"));
    assert!(!text.contains("fixture"));
    assert!(!text.contains("SELECT"));
}
