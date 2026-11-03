use mediaatlas_infrastructure::{browse::BrowseService, scan_service::ScanService};

pub struct DesktopState {
    pub browse: BrowseService,
    pub scans: ScanService,
    pub providers: mediaatlas_infrastructure::providers::ProviderService,
}
