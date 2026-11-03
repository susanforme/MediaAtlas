use mediaatlas_desktop_bridge::{AppInfoDto, ErrorDto};

#[tauri::command]
pub fn get_app_info() -> Result<AppInfoDto, ErrorDto> {
    mediaatlas_application::get_app_info(env!("CARGO_PKG_VERSION"))
        .map(Into::into)
        .map_err(Into::into)
}
