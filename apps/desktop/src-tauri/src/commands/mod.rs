use crate::state::DesktopState;
use mediaatlas_application::library::*;
use mediaatlas_desktop_bridge::{AppInfoDto, ErrorDto};
use mediaatlas_infrastructure::library::RepositoryError;
use tauri::State;
use uuid::Uuid;

fn storage_error(error: RepositoryError) -> ErrorDto {
    let code = match error {
        RepositoryError::InvalidInput => "invalid_input",
        RepositoryError::NotFound => "not_found",
        RepositoryError::Conflict => "conflict",
        RepositoryError::ProtectedMetadata => "protected_metadata",
        RepositoryError::Database => "database_error",
        RepositoryError::Migration => "migration_error",
        RepositoryError::CorruptData => "corrupt_data",
    };
    ErrorDto {
        schema_version: 1,
        code,
        message: "The local library request could not be completed.",
        correlation_id: Uuid::new_v4().to_string(),
    }
}

#[tauri::command]
pub fn get_app_info() -> Result<AppInfoDto, ErrorDto> {
    mediaatlas_application::get_app_info(env!("CARGO_PKG_VERSION"))
        .map(Into::into)
        .map_err(Into::into)
}

#[tauri::command]
pub async fn library_list_roots(
    state: State<'_, DesktopState>,
    input: RootListInput,
) -> Result<ApiResponse<Vec<RootView>>, ErrorDto> {
    state
        .browse
        .list_roots(input.include_adult)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn library_save_root(
    state: State<'_, DesktopState>,
    input: RootInput,
) -> Result<ApiResponse<RootView>, ErrorDto> {
    state
        .browse
        .save_root(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn library_remove_root(
    state: State<'_, DesktopState>,
    input: RemoveRootInput,
) -> Result<ApiResponse<bool>, ErrorDto> {
    state
        .browse
        .remove_root(input.root_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn library_list_media(
    state: State<'_, DesktopState>,
    input: BrowseQuery,
) -> Result<ApiResponse<MediaPageView>, ErrorDto> {
    state
        .browse
        .list_media(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn library_get_media(
    state: State<'_, DesktopState>,
    input: MediaDetailInput,
) -> Result<ApiResponse<MediaDetailView>, ErrorDto> {
    state
        .browse
        .get_media(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn library_summary(
    state: State<'_, DesktopState>,
    input: BrowseQuery,
) -> Result<ApiResponse<LibrarySummaryView>, ErrorDto> {
    state
        .browse
        .summary(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn library_facets(
    state: State<'_, DesktopState>,
    input: BrowseQuery,
) -> Result<ApiResponse<LibraryFacetsView>, ErrorDto> {
    state
        .browse
        .facets(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn scan_start(
    state: State<'_, DesktopState>,
    input: StartInput,
) -> Result<ApiResponse<ScanTaskView>, ErrorDto> {
    state
        .scans
        .start(input.root_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn scan_list_tasks(
    state: State<'_, DesktopState>,
    input: ListTasksInput,
) -> Result<ApiResponse<Vec<ScanTaskView>>, ErrorDto> {
    state
        .scans
        .list_tasks(input.include_adult)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn scan_get_preview(
    state: State<'_, DesktopState>,
    input: PreviewInput,
) -> Result<ApiResponse<PreviewPageView>, ErrorDto> {
    state
        .scans
        .preview(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn scan_confirm(
    state: State<'_, DesktopState>,
    input: ConfirmInput,
) -> Result<ApiResponse<ConfirmResult>, ErrorDto> {
    state
        .scans
        .confirm(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn scan_cancel(
    state: State<'_, DesktopState>,
    input: CancelInput,
) -> Result<ApiResponse<ScanTaskView>, ErrorDto> {
    state
        .scans
        .cancel(input.task_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderEnabledInput {
    provider_id: String,
    enabled: bool,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderStartInput {
    provider_id: String,
    query: String,
    language: String,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCancelInput {
    task_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderConfigInput {
    provider_id: String,
    config: mediaatlas_domain::ProviderConfig,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderSnapshotInput {}

#[tauri::command]
pub async fn provider_snapshot(
    state: State<'_, DesktopState>,
    input: ProviderSnapshotInput,
) -> Result<ApiResponse<mediaatlas_domain::ProviderSnapshot>, ErrorDto> {
    let _ = input;
    state
        .providers
        .snapshot()
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn provider_set_enabled(
    state: State<'_, DesktopState>,
    input: ProviderEnabledInput,
) -> Result<ApiResponse<()>, ErrorDto> {
    state
        .providers
        .set_enabled(&input.provider_id, input.enabled)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn provider_update_config(
    state: State<'_, DesktopState>,
    input: ProviderConfigInput,
) -> Result<ApiResponse<()>, ErrorDto> {
    state
        .providers
        .update_config(&input.provider_id, &input.config)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn provider_start(
    state: State<'_, DesktopState>,
    input: ProviderStartInput,
) -> Result<ApiResponse<Uuid>, ErrorDto> {
    state
        .providers
        .start_localized(&input.provider_id, &input.query, &input.language)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn provider_cancel(
    state: State<'_, DesktopState>,
    input: ProviderCancelInput,
) -> Result<ApiResponse<bool>, ErrorDto> {
    state
        .providers
        .cancel(input.task_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
