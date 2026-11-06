use mediaatlas_application::library::ApiResponse;
use mediaatlas_desktop_bridge::ErrorDto;
use mediaatlas_infrastructure::nfo_preview::{is_nfo, read_preview_with_artwork, NfoDocument};
use std::{collections::HashMap, path::PathBuf, sync::Mutex};
use tauri::Manager;
use uuid::Uuid;

#[derive(Default)]
struct PreviewWindows {
    paths: HashMap<String, PathBuf>,
    main_decided: bool,
}

impl PreviewWindows {
    #[cfg(any(target_os = "macos", test))]
    fn assign(&mut self, path: PathBuf) -> String {
        let label = if !self.main_decided {
            "main".to_owned()
        } else {
            format!("nfo-{}", Uuid::new_v4())
        };
        self.main_decided = true;
        self.paths.insert(label.clone(), path);
        label
    }
}

pub struct NfoState {
    windows: Mutex<PreviewWindows>,
    initialized: Mutex<bool>,
}

impl NfoState {
    pub fn from_args() -> Self {
        let pending = std::env::args_os().skip(1).map(PathBuf::from).find(|path| is_nfo(path));
        let mut windows = PreviewWindows::default();
        if let Some(path) = pending {
            windows.paths.insert("main".into(), path);
            windows.main_decided = true;
        }
        Self { windows: Mutex::new(windows), initialized: Mutex::new(false) }
    }

    #[cfg(target_os = "macos")]
    pub fn receive(&self, app: &tauri::AppHandle, urls: Vec<tauri::Url>) {
        for path in urls.into_iter().filter_map(|url| url.to_file_path().ok()).filter(|path| is_nfo(path)) {
            let title = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
            let Ok(mut windows) = self.windows.lock() else { return; };
            let label = windows.assign(path);
            drop(windows);
            // The cold-start window has not selected its frontend yet; all later files get a new window.
            if label == "main" { continue; }
            let result = tauri::WebviewWindowBuilder::new(app, &label, tauri::WebviewUrl::App("index.html".into()))
                .title(title).inner_size(1120.0, 760.0).min_inner_size(480.0, 360.0)
                .decorations(true).build();
            if result.is_err() {
                self.forget(&label);
                eprintln!("The NFO preview window could not be opened.");
            }
        }
    }

    pub fn forget(&self, label: &str) {
        if let Ok(mut windows) = self.windows.lock() { windows.paths.remove(label); }
    }
}

fn failure(code: &'static str) -> ErrorDto {
    ErrorDto { schema_version: 1, code, message: "The selected NFO could not be previewed.", correlation_id: Uuid::new_v4().to_string() }
}

#[tauri::command]
pub async fn nfo_startup_mode(window: tauri::WebviewWindow) -> Result<ApiResponse<bool>, ErrorDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let state = window.state::<NfoState>();
        let mut windows = state.windows.lock().map_err(|_| failure("internal_error"))?;
        let preview = windows.paths.get(window.label()).cloned();
        if window.label() == "main" { windows.main_decided = true; }
        drop(windows);
        if let Some(path) = preview {
            window.set_decorations(true).map_err(|_| failure("internal_error"))?;
            #[cfg(target_os = "macos")]
            window.set_title_bar_style(tauri::TitleBarStyle::Visible).map_err(|_| failure("internal_error"))?;
            window.set_title(&path.file_name().unwrap_or_default().to_string_lossy())
                .map_err(|_| failure("internal_error"))?;
            return Ok(ApiResponse::new(true));
        }
        if window.label() != "main" { return Err(failure("invalid_input")); }
        let mut initialized = state.initialized.lock().map_err(|_| failure("internal_error"))?;
        if !*initialized {
            crate::initialize_desktop(window.app_handle()).map_err(|_| failure("internal_error"))?;
            *initialized = true;
        }
        Ok(ApiResponse::new(false))
    }).await.map_err(|_| failure("internal_error"))?
}

#[tauri::command]
pub async fn nfo_take_opened(window: tauri::WebviewWindow) -> Result<ApiResponse<Option<NfoDocument>>, ErrorDto> {
    let path = window.state::<NfoState>().windows.lock().map_err(|_| failure("internal_error"))?
        .paths.get(window.label()).cloned();
    let Some(path) = path else { return Ok(ApiResponse::new(None)); };
    let path = if path.is_absolute() { path } else {
        std::env::current_dir().map_err(|_| failure("local_file_unreadable"))?.join(path)
    };
    read_preview_with_artwork(path).await.map(Some).map(ApiResponse::new).map_err(failure)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn associates_each_open_with_its_own_window_and_retains_the_previous_file() {
        let mut windows = PreviewWindows::default();
        let first = windows.assign("first.nfo".into());
        let second = windows.assign("second.nfo".into());
        let third = windows.assign("third.nfo".into());
        assert_eq!(first, "main");
        assert!(second.starts_with("nfo-"));
        assert_ne!(second, third);
        assert_eq!(windows.paths[&first], PathBuf::from("first.nfo"));
        assert_eq!(windows.paths[&second], PathBuf::from("second.nfo"));
    }

    #[test]
    fn an_existing_library_window_never_receives_a_preview() {
        let mut windows = PreviewWindows { main_decided: true, ..PreviewWindows::default() };
        assert_ne!(windows.assign("movie.nfo".into()), "main");
        assert!(!windows.paths.contains_key("main"));
    }
}
