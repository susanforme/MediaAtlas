mod support;

use chrono::Utc;
use mediaatlas_domain::{LibraryRoot, MediaKind, ProviderConfig, TaskState};
use mediaatlas_infrastructure::{
    adult_privacy::{AdultPrivacyService, AdultSettingsInput},
    adult_providers::AdultProviderHub,
    adult_scraper::AdultScraperService,
    credentials::CredentialStore,
    library::{LibraryRepository, ListMediaQuery},
    providers::{ProviderService, TypedVideoRegistration},
    scan_service::ScanService,
    scraper::ScraperService,
    video_providers::VideoProviderHub,
};
use std::{path::Path, sync::Arc, time::Duration};
use uuid::Uuid;

async fn settle(service: &ScanService, id: Uuid) {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let task = service
                .list_tasks(true)
                .await
                .unwrap()
                .into_iter()
                .find(|task| task.id == id)
                .unwrap();
            if !matches!(task.state, TaskState::Queued | TaskState::Running) {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
async fn root(repo: &LibraryRepository, path: &Path, kind: MediaKind) -> LibraryRoot {
    std::fs::create_dir_all(path).unwrap();
    let now = Utc::now();
    let root = LibraryRoot {
        id: Uuid::new_v4(),
        name: "External metadata fixture".into(),
        path: support::canonical_root_path(path).unwrap(),
        path_style: support::native_path_style(),
        kind: Some(kind),
        enabled: true,
        created_at: now,
        updated_at: now,
    };
    repo.save_root(&root).await.unwrap();
    root
}
async fn assert_no_remote_work(repo: &LibraryRepository) {
    for table in [
        "video_scrape_sessions",
        "adult_scrape_sessions",
        "provider_requests",
    ] {
        let count: i64 = sqlx::query_scalar(&format!("SELECT count(*) FROM {table}"))
            .fetch_one(repo.pool())
            .await
            .unwrap();
        assert_eq!(count, 0, "local NFO must not create {table}");
    }
}
#[tokio::test]
async fn first_scan_indexes_foreign_nfo_before_any_provider_and_refresh_does_not_rescrape() {
    let dir = support::tempdir().unwrap();
    let repo = LibraryRepository::open(&dir.path().join("db.sqlite"))
        .await
        .unwrap();
    let root = root(&repo, &dir.path().join("media"), MediaKind::Movie).await;
    let xml = r#"<?xml version="1.0"?><!-- Produced by another scraper --><movie><title>External source title</title><originaltitle>Foreign original</originaltitle><year>2024</year><plot>Existing local description</plot><tmdbid>1234</tmdbid><imdbid>tt1234</imdbid><unknown>preserve me</unknown></movie>"#;
    std::fs::write(
        Path::new(&root.path).join("Wrong filename 1990.mkv"),
        b"unchanged video",
    )
    .unwrap();
    std::fs::write(Path::new(&root.path).join("Wrong filename 1990.nfo"), xml).unwrap();
    let providers = ProviderService::new(repo.clone()).await.unwrap();
    for descriptor in VideoProviderHub::descriptors() {
        providers
            .register_adapter(Arc::new(TypedVideoRegistration(descriptor)))
            .await
            .unwrap();
    }
    providers
        .update_config(
            "tmdb",
            &ProviderConfig {
                enabled: true,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let scraper = ScraperService::new(
        repo.clone(),
        providers.clone(),
        Arc::new(VideoProviderHub::new(
            providers.engine(),
            Arc::new(CredentialStore::new()),
        )),
        providers.engine(),
        dir.path().join("cache"),
    );
    let scans = ScanService::new(repo.clone()).with_scraper(scraper);
    for _ in 0..2 {
        let task = scans.start_automatic(root.id).await.unwrap();
        settle(&scans, task.id).await;
        let items = repo
            .list_media(&ListMediaQuery::default())
            .await
            .unwrap()
            .items;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].title, "External source title");
        assert_eq!(items[0].year, Some(2024));
        assert_eq!(items[0].original_title.as_deref(), Some("Foreign original"));
        let metadata = repo.list_metadata_values(items[0].id).await.unwrap();
        assert!(metadata
            .iter()
            .any(|value| value.field_key == "overview"
                && value.value == "Existing local description"));
        let ids = repo.list_external_ids(items[0].id).await.unwrap();
        assert!(ids
            .iter()
            .any(|id| id.namespace == "tmdb" && id.external_id == "1234"));
        let source: String = sqlx::query_scalar(
            "SELECT source_kind FROM local_metadata_provenance WHERE media_item_id=?",
        )
        .bind(items[0].id.to_string())
        .fetch_one(repo.pool())
        .await
        .unwrap();
        assert_eq!(source, "nfo");
        assert_no_remote_work(&repo).await;
        assert_eq!(
            std::fs::read_to_string(Path::new(&root.path).join("Wrong filename 1990.nfo")).unwrap(),
            xml
        );
    }
    repo.pool().close().await;
}

#[tokio::test]
async fn first_adult_scan_indexes_foreign_nfo_in_its_granted_lane_without_catalog_requests() {
    let dir = support::tempdir().unwrap();
    let repo = LibraryRepository::open(&dir.path().join("db.sqlite"))
        .await
        .unwrap();
    let root = root(&repo, &dir.path().join("media"), MediaKind::AdultVideo).await;
    let xml = "<movie><title>Private external title</title><year>2025</year><plot>Private local metadata</plot><uniqueid type=\"unrelated-producer\">opaque-local-id</uniqueid></movie>";
    std::fs::write(
        Path::new(&root.path).join("SYN-001.mp4"),
        b"unchanged private video",
    )
    .unwrap();
    std::fs::write(Path::new(&root.path).join("SYN-001.nfo"), xml).unwrap();
    let providers = ProviderService::new(repo.clone()).await.unwrap();
    for descriptor in AdultProviderHub::descriptors() {
        providers
            .register_adult_descriptor(descriptor)
            .await
            .unwrap();
    }
    let privacy = AdultPrivacyService::new(repo.clone(), providers.clone());
    privacy
        .save(AdultSettingsInput {
            entry_visible: true,
            covers_visible: true,
            remember_recent: false,
        })
        .await
        .unwrap();
    privacy.unlock().await.unwrap();
    let grant = privacy.access().await.unwrap();
    let scraper = AdultScraperService::new(
        repo.clone(),
        providers.clone(),
        Arc::new(AdultProviderHub::new(
            providers.engine(),
            Arc::new(CredentialStore::new()),
        )),
        providers.engine(),
        dir.path().join("adult-cache"),
    );
    let scans = ScanService::new(repo.clone());
    for _ in 0..2 {
        let task = scans
            .start_automatic_adult(root.id, scraper.clone(), grant.clone())
            .await
            .unwrap();
        settle(&scans, task.id).await;
        let title: String = sqlx::query_scalar(
            "SELECT title FROM media_items WHERE kind='adult_video' AND adult_isolated=1",
        )
        .fetch_one(repo.pool())
        .await
        .unwrap();
        assert_eq!(title, "Private external title");
        assert_no_remote_work(&repo).await;
        assert_eq!(
            std::fs::read_to_string(Path::new(&root.path).join("SYN-001.nfo")).unwrap(),
            xml
        );
        assert!(mediaatlas_infrastructure::browse::BrowseService::new(
            repo.clone(),
            dir.path().join("cache")
        )
        .list_media(&Default::default())
        .await
        .unwrap()
        .items
        .is_empty());
    }
    repo.pool().close().await;
}

#[tokio::test]
async fn first_tv_scan_keeps_show_and_episode_metadata_and_identifiers_on_their_own_nodes() {
    let dir = support::tempdir().unwrap();
    let repo = LibraryRepository::open(&dir.path().join("db.sqlite"))
        .await
        .unwrap();
    let root = root(&repo, &dir.path().join("media"), MediaKind::TvSeries).await;
    let base = Path::new(&root.path);
    std::fs::create_dir_all(base.join("Show/S10")).unwrap();
    std::fs::write(
        base.join("Show/tvshow.nfo"),
        "<tvshow><title>Foreign Show</title><year>2022</year><tmdbid>show-42</tmdbid></tvshow>",
    )
    .unwrap();
    std::fs::write(base.join("Show/S10/Name.S10E01.mkv"), b"video").unwrap();
    std::fs::write(base.join("Show/S10/Name.S10E01.nfo"), "<episodedetails><title>Foreign Episode</title><uniqueid type=\"tmdb\">episode-99</uniqueid></episodedetails>").unwrap();
    let providers = ProviderService::new(repo.clone()).await.unwrap();
    let scraper = ScraperService::new(
        repo.clone(),
        providers.clone(),
        Arc::new(VideoProviderHub::new(
            providers.engine(),
            Arc::new(CredentialStore::new()),
        )),
        providers.engine(),
        dir.path().join("cache"),
    );
    let scans = ScanService::new(repo.clone()).with_scraper(scraper);
    for _ in 0..2 {
        let task = scans.start_automatic(root.id).await.unwrap();
        settle(&scans, task.id).await;
        let items = repo
            .list_media(&ListMediaQuery::default())
            .await
            .unwrap()
            .items;
        let show = items
            .iter()
            .find(|item| item.kind == MediaKind::TvSeries)
            .unwrap();
        let episode = items
            .iter()
            .find(|item| item.kind == MediaKind::TvEpisode)
            .unwrap();
        assert_eq!(show.title, "Foreign Show");
        assert_eq!(show.year, Some(2022));
        assert_eq!(episode.title, "Foreign Episode");
        assert!(repo
            .list_external_ids(show.id)
            .await
            .unwrap()
            .iter()
            .any(|id| id.external_id == "show-42"));
        assert!(!repo
            .list_external_ids(episode.id)
            .await
            .unwrap()
            .iter()
            .any(|id| id.external_id == "show-42"));
        assert!(repo
            .list_external_ids(episode.id)
            .await
            .unwrap()
            .iter()
            .any(|id| id.external_id == "episode-99"));
        assert!(repo
            .list_files(show.id)
            .await
            .unwrap()
            .iter()
            .any(|file| file.relative_path == "Show/tvshow.nfo"));
        assert_no_remote_work(&repo).await;
    }
    repo.pool().close().await;
}

#[tokio::test]
async fn adding_or_renaming_a_show_nfo_title_preserves_the_existing_series_identity() {
    let dir = support::tempdir().unwrap();
    let repo = LibraryRepository::open(&dir.path().join("db.sqlite"))
        .await
        .unwrap();
    let root = root(&repo, &dir.path().join("media"), MediaKind::TvSeries).await;
    let base = Path::new(&root.path);
    std::fs::create_dir_all(base.join("Show/S10")).unwrap();
    std::fs::write(base.join("Show/S10/Stable.Name.S10E01.mkv"), b"video").unwrap();
    let scans = ScanService::new(repo.clone());
    let first = scans.start_automatic(root.id).await.unwrap();
    settle(&scans, first.id).await;
    let original: String = sqlx::query_scalar("SELECT id FROM media_items WHERE kind='tv_series'")
        .fetch_one(repo.pool())
        .await
        .unwrap();
    for title in [
        "External localized title",
        "Edited external localized title",
    ] {
        std::fs::write(
            base.join("Show/tvshow.nfo"),
            format!("<tvshow><title>{title}</title><year>2024</year></tvshow>"),
        )
        .unwrap();
        let task = scans.start_automatic(root.id).await.unwrap();
        settle(&scans, task.id).await;
        let shows: Vec<(String, String)> =
            sqlx::query_as("SELECT id,title FROM media_items WHERE kind='tv_series'")
                .fetch_all(repo.pool())
                .await
                .unwrap();
        assert_eq!(shows, vec![(original.clone(), title.to_owned())]);
        let parents: i64 =
            sqlx::query_scalar("SELECT count(*) FROM media_relations WHERE relation_type='season'")
                .fetch_one(repo.pool())
                .await
                .unwrap();
        assert_eq!(
            parents, 1,
            "display metadata changes must not duplicate show graphs"
        );
        assert_no_remote_work(&repo).await;
    }
    repo.pool().close().await;
}
