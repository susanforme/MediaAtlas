mod commands;

use mediaatlas_desktop_bridge::{AppInfoDto, APP_READY_EVENT};
use std::io;
use tauri::{Emitter, Manager};

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let log_dir = app.path().app_log_dir().map_err(|_| {
                io::Error::other("Application log directory could not be resolved.")
            })?;
            let guard = mediaatlas_infrastructure::init_logging(&log_dir)?;
            app.manage(guard);
            mediaatlas_infrastructure::log_app_ready();
            let info = mediaatlas_application::get_app_info(env!("CARGO_PKG_VERSION"))?;
            app.emit(APP_READY_EVENT, AppInfoDto::from(info))?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::get_app_info])
        .run(tauri::generate_context!())
        .expect("Desktop application could not be started");
}
