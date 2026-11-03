use mediaatlas_infrastructure::{browse::BrowseService, scan_service::ScanService};

pub struct DesktopState {
    pub auth: mediaatlas_infrastructure::provider_auth::ProviderAuthService,
    pub scraper: mediaatlas_infrastructure::scraper::ScraperService,
    pub output: mediaatlas_infrastructure::media_output::MediaOutputService,
    pub browse: BrowseService,
    pub scans: ScanService,
    pub providers: mediaatlas_infrastructure::providers::ProviderService,
}
