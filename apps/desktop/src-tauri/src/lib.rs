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

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
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
            let desktop = tauri::async_runtime::block_on(async {
                let repository = LibraryRepository::open(&data_dir.join("library.sqlite")).await?;
                let scans = ScanService::new(repository.clone());
                let providers =
                    mediaatlas_infrastructure::providers::ProviderService::new(repository.clone())
                        .await?;
                let engine = providers.engine();
                let credentials = std::sync::Arc::new(
                    mediaatlas_infrastructure::credentials::CredentialStore::new(),
                );
                // An injected cloud binding is session-only and never copied into native key storage.
                // It does not authorize trusting an inherited proxy or relaxing NetworkEngine policy.
                let auth = mediaatlas_infrastructure::provider_auth::ProviderAuthService::new(
                    repository.clone(),
                    credentials.clone(),
                );
                let hub = mediaatlas_infrastructure::video_providers::VideoProviderHub::new(
                    engine.clone(),
                    credentials.clone(),
                )
                .with_auth_service(auth.clone());
                // Preserve the user's existing cloud variable name; never persist an injected proxy secret.
                if let Ok(value) = std::env::var("TMDB_PRIVATE_TOKEN")
                    .or_else(|_| std::env::var("TMDB_READ_ACCESS_TOKEN"))
                {
                    if !value.is_empty() {
                        auth.configure_session_binding(
                            "tmdb",
                            mediaatlas_infrastructure::network::SecretString::new(value),
                        )
                        .await?;
                    }
                }
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
                Ok::<_, mediaatlas_infrastructure::library::RepositoryError>(state::DesktopState {
                    repository: repository.clone(),
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
            app.manage(desktop);
            mediaatlas_infrastructure::log_app_ready();
            let info = mediaatlas_application::get_app_info(env!("CARGO_PKG_VERSION"))?;
            app.emit(APP_READY_EVENT, AppInfoDto::from(info))?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
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
