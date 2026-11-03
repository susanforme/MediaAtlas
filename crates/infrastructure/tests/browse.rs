use chrono::{DateTime, Duration, Utc};
use mediaatlas_application::library::{BrowseQuery, MediaDetailInput, RootInput};
use mediaatlas_domain::{
    Artwork, ArtworkKind, ExternalId, MediaFile, MediaItem, MediaKind, MetadataValue,
};
use mediaatlas_infrastructure::{
    browse::BrowseService,
    library::{LibraryRepository, MetadataWriteMode, RepositoryError},
};
use serde_json::json;
use std::path::Path;
use tempfile::{tempdir, TempDir};
use uuid::Uuid;

fn now() -> DateTime<Utc> {
    DateTime::from_timestamp_millis(1_700_000_000_000).unwrap()
}
async fn setup() -> (TempDir, LibraryRepository, BrowseService) {
    let dir = tempdir().unwrap();
    let repo = LibraryRepository::open(&dir.path().join("test.sqlite"))
        .await
        .unwrap();
    let service = BrowseService::new(repo.clone(), dir.path().join("cache"));
    (dir, repo, service)
}
fn input(path: &Path, kind: Option<MediaKind>) -> RootInput {
    RootInput {
        id: None,
        name: "Fixture root".into(),
        path: path.to_str().unwrap().into(),
        kind,
        enabled: true,
        exclude_patterns: vec!["**/ignored/**".into()],
    }
}
fn item(id: u128, kind: MediaKind) -> MediaItem {
    MediaItem {
        id: Uuid::from_u128(id),
        kind,
        title: format!("Fixture {}", id % 20),
        original_title: None,
        year: if id.is_multiple_of(3) {
            None
        } else {
            Some(2000 + (id % 10) as i32)
        },
        created_at: now() - Duration::milliseconds(id as i64),
        updated_at: now() + Duration::milliseconds((id % 11) as i64),
    }
}
async fn file(repo: &LibraryRepository, media: &MediaItem, root_id: Uuid, path: &str) {
    repo.save_file(&MediaFile {
        id: Uuid::new_v4(),
        media_item_id: media.id,
        root_id,
        relative_path: path.into(),
        size_bytes: 0,
        modified_at: None,
        part: None,
    })
    .await
    .unwrap();
}
async fn field(repo: &LibraryRepository, media: &MediaItem, key: &str, value: serde_json::Value) {
    repo.save_metadata_value(
        &MetadataValue {
            id: Uuid::new_v4(),
            media_item_id: media.id,
            field_key: key.into(),
            locale: None,
            value,
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
}

#[tokio::test]
async fn root_settings_are_transactional_and_removing_root_keeps_source_and_media() {
    let (dir, repo, service) = setup().await;
    let media_dir = dir.path().join("media");
    std::fs::create_dir(&media_dir).unwrap();
    let source = media_dir.join("movie.mkv");
    std::fs::write(&source, b"fixture").unwrap();
    let mut original = input(&media_dir, Some(MediaKind::Movie));
    let saved = service.save_root(&original).await.unwrap();
    assert_eq!(saved.exclude_patterns, original.exclude_patterns);
    assert_eq!(service.list_roots(false).await.unwrap().len(), 1);
    original.id = Some(saved.root.id);
    original.name = "Renamed".into();
    original.enabled = false;
    original.exclude_patterns = vec!["*.tmp".into()];
    let updated = service.save_root(&original).await.unwrap();
    assert_eq!(updated.root.created_at, saved.root.created_at);
    assert!(!updated.root.enabled);
    original.name = "Must not persist".into();
    original.exclude_patterns = vec!["[".into()];
    assert_eq!(
        service.save_root(&original).await.unwrap_err(),
        RepositoryError::InvalidInput
    );
    assert_eq!(
        service.list_roots(false).await.unwrap()[0].root.name,
        "Renamed"
    );
    let media = item(1, MediaKind::Movie);
    repo.save_media(&media).await.unwrap();
    file(&repo, &media, saved.root.id, "movie.mkv").await;
    assert!(service.remove_root(saved.root.id).await.unwrap());
    assert_eq!(std::fs::read(source).unwrap(), b"fixture");
    assert!(repo.get_media(media.id).await.unwrap().is_some());
    assert!(repo.list_files(media.id).await.unwrap().is_empty());
    repo.pool().close().await;
}

#[tokio::test]
async fn adult_kind_and_adult_root_are_hidden_in_lists_details_summary_and_facets() {
    let (dir, repo, service) = setup().await;
    let ordinary = dir.path().join("ordinary");
    let adult = dir.path().join("adult");
    std::fs::create_dir(&ordinary).unwrap();
    std::fs::create_dir(&adult).unwrap();
    let ordinary = service
        .save_root(&input(&ordinary, Some(MediaKind::Movie)))
        .await
        .unwrap();
    let adult = service
        .save_root(&input(&adult, Some(MediaKind::AdultVideo)))
        .await
        .unwrap();
    let visible = item(1, MediaKind::Movie);
    let hidden = item(2, MediaKind::AdultVideo);
    let hidden_by_root = item(3, MediaKind::Movie);
    for media in [&visible, &hidden, &hidden_by_root] {
        repo.save_media(media).await.unwrap();
    }
    file(&repo, &visible, ordinary.root.id, "ordinary.mkv").await;
    file(&repo, &hidden, adult.root.id, "adult.mkv").await;
    file(&repo, &hidden_by_root, adult.root.id, "also-adult.mkv").await;
    field(&repo, &hidden, "genres", json!(["Hidden genre"])).await;
    field(&repo, &visible, "genres", json!(["Visible genre"])).await;
    assert_eq!(service.list_roots(false).await.unwrap().len(), 1);
    assert_eq!(service.list_roots(true).await.unwrap().len(), 2);
    let page = service.list_media(&BrowseQuery::default()).await.unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].media.id, visible.id);
    assert_eq!(
        service
            .summary(&BrowseQuery::default())
            .await
            .unwrap()
            .total_media,
        1
    );
    let facets = service.facets(&BrowseQuery::default()).await.unwrap();
    assert_eq!(facets.genres[0].value, "Visible genre");
    assert_eq!(facets.genres.len(), 1);
    for media in [&hidden, &hidden_by_root] {
        assert_eq!(
            service
                .get_media(&MediaDetailInput {
                    media_id: media.id,
                    include_adult: false
                })
                .await
                .unwrap_err(),
            RepositoryError::NotFound
        );
    }
    assert_eq!(
        service
            .list_media(&BrowseQuery {
                include_adult: true,
                ..Default::default()
            })
            .await
            .unwrap()
            .items
            .len(),
        3
    );
    repo.pool().close().await;
}

#[tokio::test]
async fn all_sort_modes_page_a_thousand_records_without_loss_or_filter_cursor_reuse() {
    let (_dir, repo, service) = setup().await;
    let mut tx = repo.pool().begin().await.unwrap();
    for id in 1..=1000 {
        let media = item(id, MediaKind::Movie);
        sqlx::query("INSERT INTO media_items(id,kind,title,original_title,year,created_at,updated_at) VALUES (?,'movie',?,NULL,?,?,?)").bind(media.id.to_string()).bind(&media.title).bind(media.year).bind(media.created_at.timestamp_millis()).bind(media.updated_at.timestamp_millis()).execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
    for sort in ["title_asc", "year_desc", "added_desc", "updated_desc"] {
        let mut query = BrowseQuery {
            sort: sort.into(),
            limit: 123,
            ..Default::default()
        };
        let mut seen = std::collections::HashSet::new();
        let mut first_cursor = None;
        loop {
            let page = service.list_media(&query).await.unwrap();
            for card in page.items {
                assert!(seen.insert(card.media.id));
            }
            if first_cursor.is_none() {
                first_cursor = page.next_cursor.clone();
            }
            match page.next_cursor {
                Some(cursor) => query.cursor = Some(cursor),
                None => break,
            }
        }
        assert_eq!(seen.len(), 1000);
        query.cursor = first_cursor;
        query.query = Some("different".into());
        assert_eq!(
            service.list_media(&query).await.unwrap_err(),
            RepositoryError::InvalidInput
        );
    }
    assert_eq!(
        service
            .list_media(&BrowseQuery {
                limit: 0,
                ..Default::default()
            })
            .await
            .unwrap_err(),
        RepositoryError::InvalidInput
    );
    repo.pool().close().await;
}

#[tokio::test]
async fn filters_health_and_local_metadata_facets_use_real_records_and_literal_search() {
    let (dir, repo, service) = setup().await;
    let root = service
        .save_root(&input(dir.path(), Some(MediaKind::Movie)))
        .await
        .unwrap();
    let mut first = item(1, MediaKind::Movie);
    first.title = "100%_Literal'".into();
    first.year = Some(2020);
    let second = item(2, MediaKind::Movie);
    repo.save_media(&first).await.unwrap();
    repo.save_media(&second).await.unwrap();
    file(&repo, &first, root.root.id, "movie.mkv").await;
    file(&repo, &first, root.root.id, "movie.nfo").await;
    field(&repo, &first, "genres", json!(["Drama", "Comedy"])).await;
    field(&repo, &first, "actors", json!(["Fixture actor"])).await;
    field(&repo, &first, "studio", json!("Fixture studio")).await;
    let query = BrowseQuery {
        root_id: Some(root.root.id),
        genre: Some("Drama".into()),
        person: Some("Fixture actor".into()),
        studio: Some("Fixture studio".into()),
        year: Some(2020),
        query: Some("%_Literal'".into()),
        ..Default::default()
    };
    let page = service.list_media(&query).await.unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].media.id, first.id);
    assert!(page.items[0].health.contains(&"missing_poster".into()));
    assert!(!page.items[0].health.contains(&"missing_nfo".into()));
    let summary = service.summary(&BrowseQuery::default()).await.unwrap();
    assert_eq!(summary.missing_nfo, 1);
    assert_eq!(summary.incomplete_metadata, 1);
    let facets = service.facets(&BrowseQuery::default()).await.unwrap();
    assert_eq!(facets.genres.len(), 2);
    assert_eq!(facets.people[0].value, "Fixture actor");
    assert_eq!(facets.studios[0].value, "Fixture studio");
    assert!(service
        .list_media(&BrowseQuery {
            query: Some("' OR 1=1 --".into()),
            ..Default::default()
        })
        .await
        .unwrap()
        .items
        .is_empty());
    let missing = service
        .list_media(&BrowseQuery {
            health: Some("incomplete_metadata".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(missing.items.len(), 1);
    assert_eq!(missing.items[0].media.id, second.id);
    for media in [&first, &second] {
        repo.save_external_id(&ExternalId {
            id: Uuid::new_v4(),
            media_item_id: media.id,
            provider_id: "fixture".into(),
            namespace: "movie".into(),
            external_id: "same-id".into(),
            is_primary: true,
            created_at: now(),
        })
        .await
        .unwrap();
    }
    assert_eq!(
        service
            .list_media(&BrowseQuery {
                health: Some("duplicate".into()),
                ..Default::default()
            })
            .await
            .unwrap()
            .items
            .len(),
        2
    );
    repo.pool().close().await;
}

#[tokio::test]
async fn poster_uses_bounded_thumbnail_cache_and_ignores_artwork_outside_root() {
    let (dir, repo, service) = setup().await;
    let root_path = dir.path().join("root");
    std::fs::create_dir(&root_path).unwrap();
    let root = service
        .save_root(&input(&root_path, Some(MediaKind::Movie)))
        .await
        .unwrap();
    let media = item(1, MediaKind::Movie);
    repo.save_media(&media).await.unwrap();
    file(&repo, &media, root.root.id, "movie.mkv").await;
    image::RgbImage::from_pixel(600, 900, image::Rgb([30, 80, 50]))
        .save(root_path.join("poster.png"))
        .unwrap();
    let mut art = Artwork {
        id: Uuid::new_v4(),
        media_item_id: media.id,
        kind: ArtworkKind::Poster,
        cache_path: None,
        source_url: Some(format!("library://{}/poster.png", root.root.id)),
        provider_id: None,
        width: None,
        height: None,
        content_hash: None,
        created_at: now(),
    };
    repo.save_artwork(&art).await.unwrap();
    let page = service.list_media(&BrowseQuery::default()).await.unwrap();
    let uri = page.items[0].poster_uri.as_ref().unwrap();
    use base64::Engine;
    let data = base64::engine::general_purpose::STANDARD
        .decode(uri.strip_prefix("data:image/png;base64,").unwrap())
        .unwrap();
    let thumbnail = image::load_from_memory(&data).unwrap();
    assert!(thumbnail.width() <= 240 && thumbnail.height() <= 360);
    assert!(dir.path().join("cache/general").is_dir());
    assert!(!dir.path().join("cache/adult").exists());
    art.source_url = Some(format!("library://{}/../outside.png", root.root.id));
    repo.save_artwork(&art).await.unwrap();
    assert!(service
        .list_media(&BrowseQuery::default())
        .await
        .unwrap()
        .items[0]
        .poster_uri
        .is_none());
    assert!(root_path.join("poster.png").exists());
    repo.pool().close().await;
}

#[tokio::test]
async fn adult_root_removal_does_not_expose_orphaned_media_in_ordinary_views() {
    let (dir, repo, service) = setup().await;
    let root_path = dir.path().join("private");
    std::fs::create_dir(&root_path).unwrap();
    let adult = service
        .save_root(&input(&root_path, Some(MediaKind::AdultVideo)))
        .await
        .unwrap();
    let media = item(1, MediaKind::Movie);
    repo.save_media(&media).await.unwrap();
    file(&repo, &media, adult.root.id, "fixture.mkv").await;
    assert!(service
        .list_media(&BrowseQuery::default())
        .await
        .unwrap()
        .items
        .is_empty());
    assert!(service.remove_root(adult.root.id).await.unwrap());
    assert!(repo.get_media(media.id).await.unwrap().is_some());
    assert!(service
        .list_media(&BrowseQuery::default())
        .await
        .unwrap()
        .items
        .is_empty());
    assert_eq!(
        service
            .summary(&BrowseQuery::default())
            .await
            .unwrap()
            .total_media,
        0
    );
    assert_eq!(
        service
            .list_media(&BrowseQuery {
                include_adult: true,
                ..Default::default()
            })
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    repo.pool().close().await;
}
