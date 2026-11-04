use mediaatlas_application::library::ApiResponse;
use mediaatlas_desktop_bridge::ErrorDto;
use mediaatlas_infrastructure::nfo_preview::{is_nfo, read_preview, NfoDocument};
use serde::Deserialize;
use std::{path::PathBuf, sync::Mutex};
#[cfg(target_os = "macos")]
use tauri::Emitter;
use tauri::{Manager, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

pub struct NfoState {
    pending: Mutex<Option<PathBuf>>,
    startup_preview: bool,
    initialized: Mutex<bool>,
}

impl NfoState {
    pub fn from_args() -> Self {
        let pending = std::env::args_os()
            .skip(1)
            .map(PathBuf::from)
            .filter(|path| is_nfo(path))
            .last();
        Self {
            startup_preview: pending.is_some(),
            pending: Mutex::new(pending),
            initialized: Mutex::new(false),
        }
    }

    #[cfg(target_os = "macos")]
    pub fn receive(&self, app: &tauri::AppHandle, urls: Vec<tauri::Url>) {
        let Some(path) = urls
            .into_iter()
            .filter_map(|url| url.to_file_path().ok())
            .find(|path| is_nfo(path))
        else {
            return;
        };
        if let Ok(mut pending) = self.pending.lock() {
            *pending = Some(path);
            let _ = app.emit("nfo-opened", ());
        }
    }
}

fn failure(code: &'static str) -> ErrorDto {
    ErrorDto {
        schema_version: 1,
        code,
        message: "The selected NFO could not be previewed.",
        correlation_id: Uuid::new_v4().to_string(),
    }
}

#[tauri::command]
pub async fn nfo_startup_mode(app: tauri::AppHandle) -> Result<ApiResponse<bool>, ErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<NfoState>();
        if state.startup_preview
            || state
                .pending
                .lock()
                .map_err(|_| failure("internal_error"))?
                .is_some()
        {
            return Ok(ApiResponse::new(true));
        }
        let mut initialized = state
            .initialized
            .lock()
            .map_err(|_| failure("internal_error"))?;
        if !*initialized {
            crate::initialize_desktop(&app).map_err(|_| failure("internal_error"))?;
            *initialized = true;
        }
        Ok(ApiResponse::new(false))
    })
    .await
    .map_err(|_| failure("internal_error"))?
}

#[tauri::command]
pub fn nfo_has_opened(state: State<'_, NfoState>) -> Result<ApiResponse<bool>, ErrorDto> {
    let pending = state.pending.lock().map_err(|_| failure("internal_error"))?;
    Ok(ApiResponse::new(pending.is_some()))
}

#[tauri::command]
pub async fn nfo_take_opened(
    app: tauri::AppHandle,
) -> Result<ApiResponse<Option<NfoDocument>>, ErrorDto> {
    let path = app
        .state::<NfoState>()
        .pending
        .lock()
        .map_err(|_| failure("internal_error"))?
        .take();
    let Some(path) = path else {
        return Ok(ApiResponse::new(None));
    };
    tauri::async_runtime::spawn_blocking(move || {
        let path = if path.is_absolute() {
            path
        } else {
            std::env::current_dir()
                .map_err(|_| failure("local_file_unreadable"))?
                .join(path)
        };
        read_preview(&path)
            .map(Some)
            .map(ApiResponse::new)
            .map_err(failure)
    })
    .await
    .map_err(|_| failure("internal_error"))?
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NfoDialogInput {
    language: String,
}

#[tauri::command]
pub async fn nfo_open_dialog(
    window: tauri::WebviewWindow,
    input: NfoDialogInput,
) -> Result<ApiResponse<Option<NfoDocument>>, ErrorDto> {
    let title = match input.language.as_str() {
        "zh-CN" => "打开 NFO 文件",
        "en-US" => "Open NFO file",
        _ => return Err(failure("invalid_input")),
    };
    let dialog = window
        .dialog()
        .file()
        .set_title(title)
        .add_filter("NFO", &["nfo"]);
    #[cfg(any(windows, target_os = "macos"))]
    let dialog = dialog.set_parent(&window);
    let Some(file) = dialog.blocking_pick_file() else {
        return Ok(ApiResponse::new(None));
    };
    let path = file.into_path().map_err(|_| failure("invalid_input"))?;
    tauri::async_runtime::spawn_blocking(move || {
        read_preview(&path)
            .map(Some)
            .map(ApiResponse::new)
            .map_err(failure)
    })
    .await
    .map_err(|_| failure("internal_error"))?
}
