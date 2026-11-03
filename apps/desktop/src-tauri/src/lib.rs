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
                scans.recover_interrupted().await?;
                Ok::<_, mediaatlas_infrastructure::library::RepositoryError>(state::DesktopState {
                    browse: BrowseService::new(repository, data_dir.join("thumbnails")),
                    scans,
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
            commands::scan_cancel
        ])
        .run(tauri::generate_context!())
        .expect("Desktop application could not be started");
}
