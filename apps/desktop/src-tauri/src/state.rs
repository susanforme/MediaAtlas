use mediaatlas_infrastructure::{browse::BrowseService, scan_service::ScanService};

pub struct DesktopState {
    pub maintenance: mediaatlas_infrastructure::maintenance::MaintenanceService,
    pub organization: mediaatlas_infrastructure::organization::OrganizationService,
    pub watcher: mediaatlas_infrastructure::watcher::WatcherService,
    pub health: mediaatlas_infrastructure::health::HealthService,
    pub video_open: mediaatlas_infrastructure::video_open::VideoOpenService,
    pub music_artwork: mediaatlas_infrastructure::music_artwork::MusicArtworkService,
    pub music_open: mediaatlas_infrastructure::music_open::MusicOpenService,
    pub music: mediaatlas_infrastructure::music_library::MusicLibraryService,
    pub music_providers: mediaatlas_infrastructure::music_providers::MusicProviderHub,
    pub music_tags: mediaatlas_infrastructure::music_tags::MusicTagService,
    pub repository: mediaatlas_infrastructure::library::LibraryRepository,
    pub adult_cache: std::path::PathBuf,
    pub adult_privacy: mediaatlas_infrastructure::adult_privacy::AdultPrivacyService,
    pub adult_scraper: mediaatlas_infrastructure::adult_scraper::AdultScraperService,
    pub adult_browse: BrowseService,
    pub auth: mediaatlas_infrastructure::provider_auth::ProviderAuthService,
    pub scraper: mediaatlas_infrastructure::scraper::ScraperService,
    pub output: mediaatlas_infrastructure::media_output::MediaOutputService,
    pub browse: BrowseService,
    pub scans: ScanService,
    pub providers: mediaatlas_infrastructure::providers::ProviderService,
}

impl DesktopState {
    pub async fn adult_output(
        &self,
    ) -> Result<
        mediaatlas_infrastructure::media_output::MediaOutputService,
        mediaatlas_infrastructure::library::RepositoryError,
    > {
        let access = self.adult_privacy.access().await?;
        Ok(
            mediaatlas_infrastructure::media_output::MediaOutputService::new_adult(
                self.repository.clone(),
                self.adult_cache.clone(),
                access,
            ),
        )
    }
    pub async fn adult_batch(
        &self,
    ) -> Result<
        mediaatlas_infrastructure::adult_output::AdultBatchService,
        mediaatlas_infrastructure::library::RepositoryError,
    > {
        Ok(
            mediaatlas_infrastructure::adult_output::AdultBatchService::new(
                self.repository.clone(),
                self.adult_output().await?,
            ),
        )
    }
}
