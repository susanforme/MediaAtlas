//! Full-library maintenance never accepts arbitrary paths and never swaps an active pool.
use super::DesktopState;
use mediaatlas_application::library::ApiResponse;
use mediaatlas_desktop_bridge::ErrorDto;
use mediaatlas_infrastructure::maintenance::*;
use tauri::State;
use uuid::Uuid;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceEmptyInput {}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaintenanceIdInput {
    id: Uuid,
}
pub(super) fn maintenance_error(error: MaintenanceError) -> ErrorDto {
    ErrorDto {
        schema_version: 1,
        code: error.code(),
        message:
            "The maintenance request could not be completed. Preserved recovery files are retained.",
        correlation_id: Uuid::new_v4().to_string(),
    }
}
#[tauri::command]
pub async fn maintenance_status(
    state: State<'_, DesktopState>,
    input: MaintenanceEmptyInput,
) -> Result<ApiResponse<MaintenanceStatus>, ErrorDto> {
    let _ = input;
    state
        .maintenance
        .status()
        .await
        .map(ApiResponse::new)
        .map_err(maintenance_error)
}
#[tauri::command]
pub async fn maintenance_backup_preview(
    state: State<'_, DesktopState>,
    input: MaintenanceEmptyInput,
) -> Result<ApiResponse<BackupPreview>, ErrorDto> {
    let _ = input;
    let access = state.adult_privacy.access().await.ok();
    state
        .maintenance
        .backup_preview(access)
        .await
        .map(ApiResponse::new)
        .map_err(maintenance_error)
}
#[tauri::command]
pub async fn maintenance_backup_confirm(
    state: State<'_, DesktopState>,
    input: MaintenanceIdInput,
) -> Result<ApiResponse<BackupSummary>, ErrorDto> {
    let access = state.adult_privacy.access().await.ok();
    state
        .maintenance
        .backup_confirm(input.id, access)
        .await
        .map(ApiResponse::new)
        .map_err(maintenance_error)
}
#[tauri::command]
pub async fn maintenance_restore_preview(
    state: State<'_, DesktopState>,
    input: MaintenanceIdInput,
) -> Result<ApiResponse<RestorePreview>, ErrorDto> {
    let access = state.adult_privacy.access().await.ok();
    state
        .maintenance
        .restore_preview(input.id, access)
        .await
        .map(ApiResponse::new)
        .map_err(maintenance_error)
}
#[tauri::command]
pub async fn maintenance_restore_confirm(
    state: State<'_, DesktopState>,
    input: MaintenanceIdInput,
) -> Result<ApiResponse<RestoreQueued>, ErrorDto> {
    let access = state.adult_privacy.access().await.ok();
    state
        .maintenance
        .restore_confirm(input.id, access)
        .await
        .map(ApiResponse::new)
        .map_err(maintenance_error)
}
#[tauri::command]
pub async fn maintenance_diagnostics_export(
    state: State<'_, DesktopState>,
    input: MaintenanceEmptyInput,
) -> Result<ApiResponse<ExportResult>, ErrorDto> {
    let _ = input;
    state
        .maintenance
        .diagnostics_export()
        .await
        .map(ApiResponse::new)
        .map_err(maintenance_error)
}
#[tauri::command]
pub async fn maintenance_cache_preview(
    state: State<'_, DesktopState>,
    input: MaintenanceEmptyInput,
) -> Result<ApiResponse<CachePreview>, ErrorDto> {
    let _ = input;
    state
        .maintenance
        .cache_preview()
        .await
        .map(ApiResponse::new)
        .map_err(maintenance_error)
}
#[tauri::command]
pub async fn maintenance_cache_confirm(
    state: State<'_, DesktopState>,
    input: MaintenanceIdInput,
) -> Result<ApiResponse<CacheResult>, ErrorDto> {
    state
        .maintenance
        .cache_confirm(input.id)
        .await
        .map(ApiResponse::new)
        .map_err(maintenance_error)
}
#[tauri::command]
pub async fn maintenance_cancel(
    state: State<'_, DesktopState>,
    input: MaintenanceEmptyInput,
) -> Result<ApiResponse<()>, ErrorDto> {
    let _ = input;
    state.maintenance.cancel();
    Ok(ApiResponse::new(()))
}
