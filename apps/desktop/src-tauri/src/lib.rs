mod commands;
mod state;

use mediaatlas_application::library::ScanTaskView;
use mediaatlas_desktop_bridge::{AppInfoDto, APP_READY_EVENT};
use mediaatlas_domain::TaskState;
use mediaatlas_infrastructure::{
    browse::BrowseService, library::LibraryRepository, scan_service::ScanService,
};
use serde::Serialize;
use std::io;
use tauri::{Emitter, Manager};
use uuid::Uuid;

#[derive(Clone, Serialize)]
struct TaskEvent {
    schema_version: u16,
    correlation_id: Uuid,
    data: ScanTaskView,
}

// Rust statics are retained until process termination. This keeps the OS lock
// beyond any unspecified Tauri managed-state or background-runtime drop order.
static DATABASE_LEASE: std::sync::OnceLock<mediaatlas_infrastructure::maintenance::DatabaseLease> =
    std::sync::OnceLock::new();

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let startup_started = std::time::Instant::now();
            let log_dir = app.path().app_log_dir().map_err(|_| {
                io::Error::other("Application log directory could not be resolved.")
            })?;
            let guard = mediaatlas_infrastructure::init_logging(&log_dir)?;
            app.manage(guard);
            let data_dir = app.path().app_data_dir().map_err(|_| {
                io::Error::other("Application data directory could not be resolved.")
            })?;
            std::fs::create_dir_all(&data_dir)
                .map_err(|_| io::Error::other("Application data directory is unavailable."))?;
            let data_dir = std::fs::canonicalize(&data_dir)
                .map_err(|_| io::Error::other("Application data directory is unavailable."))?;
            let database_path = data_dir.join("library.sqlite");
            let maintenance_dir = data_dir.join("maintenance");
            let lease =
                mediaatlas_infrastructure::maintenance::DatabaseLease::acquire(&database_path)
                    .map_err(|_| io::Error::other("The library is already open or unavailable."))?;
            DATABASE_LEASE
                .set(lease)
                .map_err(|_| io::Error::other("The library process was already initialized."))?;
            let lease = DATABASE_LEASE.get().ok_or_else(|| {
                io::Error::other("The library process could not acquire its lifetime lock.")
            })?;
            tauri::async_runtime::block_on(lease.apply_pending_restore(&maintenance_dir)).map_err(
                |_| {
                    io::Error::other(
                        "Database recovery requires attention; all recovery files are retained.",
                    )
                },
            )?;
            // The process-static lease outlives every pool/service/runtime handle.
            let desktop = tauri::async_runtime::block_on(async {
                let repository = LibraryRepository::open(&database_path).await?;
                let maintenance = mediaatlas_infrastructure::maintenance::MaintenanceService::new(
                    repository.clone(),
                    database_path.clone(),
                    maintenance_dir.clone(),
                    data_dir.join("artwork-cache").join("general"),
                )
                .map_err(|_| mediaatlas_infrastructure::library::RepositoryError::Database)?;
                let providers =
                    mediaatlas_infrastructure::providers::ProviderService::new(repository.clone())
                        .await?;
                let engine = providers.engine();
                let credentials = std::sync::Arc::new(
                    mediaatlas_infrastructure::credentials::CredentialStore::new(),
                );
                let auth = mediaatlas_infrastructure::provider_auth::ProviderAuthService::new(
                    repository.clone(),
                    credentials.clone(),
                );
                let hub = mediaatlas_infrastructure::video_providers::VideoProviderHub::new(
                    engine.clone(),
                    credentials.clone(),
                )
                .with_auth_service(auth.clone());
                for descriptor in
                    mediaatlas_infrastructure::video_providers::VideoProviderHub::descriptors()
                {
                    providers
                        .register_adapter(std::sync::Arc::new(
                            mediaatlas_infrastructure::providers::TypedVideoRegistration(
                                descriptor,
                            ),
                        ))
                        .await?;
                }
                let cache = data_dir.join("artwork-cache").join("general");
                let output = mediaatlas_infrastructure::media_output::MediaOutputService::new(
                    repository.clone(),
                    cache.clone(),
                );
                output.recover_interrupted().await?;
                let scraper = mediaatlas_infrastructure::scraper::ScraperService::new(
                    repository.clone(),
                    providers.clone(),
                    std::sync::Arc::new(hub),
                    engine.clone(),
                    cache,
                )
                .with_auth_service(auth.clone());
                scraper.initialize().await?;
                let scans = ScanService::new(repository.clone()).with_scraper(scraper.clone());
                let music = mediaatlas_infrastructure::music_library::MusicLibraryService::new(
                    repository.clone(),
                );
                for descriptor in
                    mediaatlas_infrastructure::music_providers::MusicProviderHub::descriptors()
                {
                    providers
                        .register_adapter(std::sync::Arc::new(
                            mediaatlas_infrastructure::providers::TypedVideoRegistration(
                                descriptor,
                            ),
                        ))
                        .await?;
                }
                let music_providers =
                    mediaatlas_infrastructure::music_providers::MusicProviderHub::new(
                        engine.clone(),
                    )
                    .with_provider_service(providers.clone());
                let music_tags =
                    mediaatlas_infrastructure::music_tags::MusicTagService::new(repository.clone());
                music_tags.recover_interrupted().await?;
                let music_artwork =
                    mediaatlas_infrastructure::music_artwork::MusicArtworkService::new(
                        repository.clone(),
                        music_providers.clone(),
                        output.clone(),
                    );
                let adult_cache = data_dir.join("artwork-cache").join("adult");
                for descriptor in
                    mediaatlas_infrastructure::adult_providers::AdultProviderHub::descriptors()
                {
                    providers.register_adult_descriptor(descriptor).await?;
                }
                let adult_hub = mediaatlas_infrastructure::adult_providers::AdultProviderHub::new(
                    engine.clone(),
                    credentials,
                )
                .with_auth_service(auth.clone());
                let adult_scraper =
                    mediaatlas_infrastructure::adult_scraper::AdultScraperService::new(
                        repository.clone(),
                        providers.clone(),
                        std::sync::Arc::new(adult_hub),
                        engine,
                        adult_cache.clone(),
                    )
                    .with_auth_service(auth.clone());
                adult_scraper.initialize().await?;
                // Recovery only records interrupted work. No adult grant or filesystem replay at startup.
                mediaatlas_infrastructure::adult_output::recover_adult_batches(&repository).await?;
                let adult_privacy =
                    mediaatlas_infrastructure::adult_privacy::AdultPrivacyService::new(
                        repository.clone(),
                        providers.clone(),
                    );
                scans.recover_interrupted().await?;
                let organization =
                    mediaatlas_infrastructure::organization::OrganizationService::new(
                        repository.clone(),
                    );
                organization.recover_interrupted().await?;
                let watcher = mediaatlas_infrastructure::watcher::WatcherService::new(
                    repository.clone(),
                    scans.clone(),
                );
                Ok::<_, mediaatlas_infrastructure::library::RepositoryError>(state::DesktopState {
                    repository: repository.clone(),
                    maintenance,
                    organization,
                    watcher,
                    health: mediaatlas_infrastructure::health::HealthService::new(
                        repository.clone(),
                    )
                    .with_artwork_cache(data_dir.join("artwork-cache").join("general")),
                    video_open: mediaatlas_infrastructure::video_open::VideoOpenService::new(
                        repository.clone(),
                    ),
                    music,
                    music_artwork,
                    music_open: mediaatlas_infrastructure::music_open::MusicOpenService::new(
                        repository.clone(),
                    ),
                    music_providers,
                    music_tags,
                    adult_cache: adult_cache.clone(),
                    adult_browse: BrowseService::new(
                        repository.clone(),
                        data_dir.join("adult-thumbnails"),
                    )
                    .with_artwork_cache(adult_cache),
                    adult_privacy,
                    adult_scraper,
                    browse: BrowseService::new(repository, data_dir.join("thumbnails"))
                        .with_artwork_cache(data_dir.join("artwork-cache").join("general")),
                    scans,
                    providers,
                    scraper,
                    output,
                    auth,
                })
            })?;
            let mut events = desktop.scans.subscribe();
            let event_browse = desktop.browse.clone();
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    let task = match events.recv().await {
                        Ok(task) => task,
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    };
                    // Adult/Mixed task details are retrieved only by explicit filtered
                    // polling. Never broadcast their counters to ordinary views.
                    let Ok(roots) = event_browse.list_roots(false).await else {
                        continue;
                    };
                    if !roots
                        .iter()
                        .any(|root| root.root.id == task.root_id && root.root.kind.is_some())
                    {
                        continue;
                    }
                    let payload = TaskEvent {
                        schema_version: 1,
                        correlation_id: task.id,
                        data: task.clone(),
                    };
                    let _ = handle.emit("task-updated", &payload);
                    let event = match task.state {
                        TaskState::Queued => "task-created",
                        TaskState::WaitingForUser => "task-needs-review",
                        TaskState::Succeeded
                        | TaskState::Failed
                        | TaskState::Cancelled
                        | TaskState::Interrupted => "task-completed",
                        _ => "task-updated",
                    };
                    if event != "task-updated" {
                        let _ = handle.emit(event, &payload);
                    }
                    if task.error_code.is_some() || task.warnings > 0 {
                        let _ = handle.emit("task-log", &payload);
                    }
                }
            });
            // Legacy NFO reads and recursive watcher registration may touch slow or
            // offline media disks. Neither is a prerequisite for rendering the UI.
            let background_repository = desktop.repository.clone();
            let background_watcher = desktop.watcher.clone();
            app.manage(desktop);
            mediaatlas_infrastructure::log_startup_stage(
                mediaatlas_infrastructure::StartupStage::Services,
                startup_started.elapsed(),
                true,
            );
            tauri::async_runtime::spawn(async move {
                let started = std::time::Instant::now();
                let result = mediaatlas_infrastructure::library_ready::backfill_local_nfo(
                    &background_repository,
                )
                .await;
                mediaatlas_infrastructure::log_startup_stage(
                    mediaatlas_infrastructure::StartupStage::LegacyMetadata,
                    started.elapsed(),
                    result.is_ok(),
                );
            });
            tauri::async_runtime::spawn(async move {
                let started = std::time::Instant::now();
                // Watcher status retains individual Root failures for the UI.
                let result = background_watcher.restore().await;
                mediaatlas_infrastructure::log_startup_stage(
                    mediaatlas_infrastructure::StartupStage::Watchers,
                    started.elapsed(),
                    result.is_ok(),
                );
            });
            mediaatlas_infrastructure::log_app_ready();
            let info = mediaatlas_application::get_app_info(env!("CARGO_PKG_VERSION"))?;
            app.emit(APP_READY_EVENT, AppInfoDto::from(info))?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::maintenance_status,
            commands::maintenance_backup_preview,
            commands::maintenance_backup_confirm,
            commands::maintenance_restore_preview,
            commands::maintenance_restore_confirm,
            commands::maintenance_diagnostics_export,
            commands::maintenance_cache_preview,
            commands::maintenance_cache_confirm,
            commands::maintenance_cancel,
            commands::music_tools_library,
            commands::music_tools_album,
            commands::organization_plan,
            commands::organization_logs,
            commands::adult_organization_plan,
            commands::adult_organization_logs,
            commands::organization_preview,
            commands::organization_execute,
            commands::organization_cancel,
            commands::organization_rollback_preview,
            commands::organization_rollback_execute,
            commands::organization_discard,
            commands::organization_history,
            commands::library_open,
            commands::library_tools_media,
            commands::library_tools_detail,
            commands::adult_organization_preview,
            commands::adult_organization_execute,
            commands::adult_organization_cancel,
            commands::adult_organization_rollback_preview,
            commands::adult_organization_rollback_execute,
            commands::adult_organization_discard,
            commands::adult_organization_history,
            commands::adult_open,
            commands::adult_tools_media,
            commands::adult_tools_detail,
            commands::watcher_list,
            commands::watcher_configure,
            commands::health_check,
            commands::music_open_file,
            commands::music_library,
            commands::music_album,
            commands::music_artist,
            commands::music_local_files,
            commands::music_search,
            commands::music_release,
            commands::music_import,
            commands::music_cover,
            commands::music_cover_preview,
            commands::music_cached_cover,
            commands::music_link_track,
            commands::music_match,
            commands::music_tag_read,
            commands::music_tag_history,
            commands::music_tag_preview,
            commands::music_tag_execute,
            commands::music_restore_preview,
            commands::music_restore_execute,
            commands::music_tag_discard,
            commands::adult_settings,
            commands::adult_save_settings,
            commands::adult_unlock,
            commands::adult_lock,
            commands::adult_search,
            commands::adult_search_preview,
            commands::adult_detail,
            commands::adult_preview,
            commands::adult_confirm,
            commands::adult_media_detail,
            commands::adult_artwork_preview,
            commands::adult_output_preview,
            commands::adult_output_execute,
            commands::adult_output_discard,
            commands::adult_batch_preview,
            commands::adult_batch_execute,
            commands::adult_batch_cancel,
            commands::adult_batch_status,
            commands::adult_providers,
            commands::adult_list_media,
            commands::adult_get_media,
            commands::adult_list_roots,
            commands::adult_tasks,
            commands::adult_recent,
            commands::library_list_roots,
            commands::library_save_root,
            commands::library_remove_root,
            commands::library_list_media,
            commands::library_get_media,
            commands::library_summary,
            commands::library_facets,
            commands::scan_start,
            commands::library_sync,
            commands::scan_list_tasks,
            commands::scan_get_preview,
            commands::scan_confirm,
            commands::scan_cancel,
            commands::provider_snapshot,
            commands::provider_set_enabled,
            commands::provider_update_config,
            commands::provider_start,
            commands::provider_cancel,
            commands::scraper_search,
            commands::scraper_detail,
            commands::scraper_preview,
            commands::scraper_confirm,
            commands::scraper_history,
            commands::scraper_output_preview,
            commands::scraper_output_execute,
            commands::scraper_artwork_preview,
            commands::scraper_search_preview,
            commands::scraper_media_detail,
            commands::scraper_output_discard,
            commands::provider_auth_status,
            commands::provider_auth_save,
            commands::provider_auth_clear
        ])
        .run(tauri::generate_context!())
        .expect("Desktop application could not be started");
}
