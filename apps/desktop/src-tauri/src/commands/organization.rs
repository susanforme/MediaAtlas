//! Typed organization, review and native opening commands. Frontend supplies IDs only.
use super::{storage_error, DesktopState};
use mediaatlas_application::library::*;
use mediaatlas_desktop_bridge::ErrorDto;
use mediaatlas_domain::MediaKind;
use mediaatlas_infrastructure::{
    health::{HealthPage, HealthQuery},
    organization::{
        OrganizationLog, OrganizationPlan, OrganizationRequest, OrganizationResult,
        OrganizationService,
    },
    video_open::{VideoOpenResult, VideoOpenService},
    watcher::WatcherStatus,
};
use tauri::State;
use uuid::Uuid;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OperationIdInput {
    id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenFileInput {
    file_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReviewEmptyInput {}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WatcherInput {
    root_id: Uuid,
    enabled: bool,
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolsMediaInput {
    media_id: Uuid,
}

#[tauri::command]
pub async fn organization_preview(
    state: State<'_, DesktopState>,
    input: OrganizationRequest,
) -> Result<ApiResponse<OrganizationPlan>, ErrorDto> {
    let _ = &input;
    let service = state.organization.clone();
    let result = service.preview(input).await.map_err(storage_error)?;

    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn organization_execute(
    state: State<'_, DesktopState>,
    input: OperationIdInput,
) -> Result<ApiResponse<OrganizationResult>, ErrorDto> {
    let _ = &input;
    let service = state.organization.clone();
    let result = service.execute(input.id).await.map_err(storage_error)?;

    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn organization_cancel(
    state: State<'_, DesktopState>,
    input: OperationIdInput,
) -> Result<ApiResponse<OrganizationResult>, ErrorDto> {
    let _ = &input;
    let service = state.organization.clone();
    let result = service.cancel(input.id).await.map_err(storage_error)?;

    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn organization_rollback_preview(
    state: State<'_, DesktopState>,
    input: OperationIdInput,
) -> Result<ApiResponse<OrganizationPlan>, ErrorDto> {
    let _ = &input;
    let service = state.organization.clone();
    let result = service
        .rollback_preview(input.id)
        .await
        .map_err(storage_error)?;

    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn organization_rollback_execute(
    state: State<'_, DesktopState>,
    input: OperationIdInput,
) -> Result<ApiResponse<OrganizationResult>, ErrorDto> {
    let _ = &input;
    let service = state.organization.clone();
    let result = service
        .rollback_execute(input.id)
        .await
        .map_err(storage_error)?;

    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn organization_discard(
    state: State<'_, DesktopState>,
    input: OperationIdInput,
) -> Result<ApiResponse<()>, ErrorDto> {
    let _ = &input;
    let service = state.organization.clone();
    service.discard(input.id).await.map_err(storage_error)?;

    Ok(ApiResponse::new(()))
}

#[tauri::command]
pub async fn organization_history(
    state: State<'_, DesktopState>,
    input: ReviewEmptyInput,
) -> Result<ApiResponse<Vec<OrganizationResult>>, ErrorDto> {
    let _ = &input;
    let service = state.organization.clone();
    let result = service.history().await.map_err(storage_error)?;

    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn library_open(
    state: State<'_, DesktopState>,
    input: OpenFileInput,
) -> Result<ApiResponse<VideoOpenResult>, ErrorDto> {
    let service = state.video_open.clone();
    service
        .open(input.file_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn library_tools_media(
    state: State<'_, DesktopState>,
    mut input: BrowseQuery,
) -> Result<ApiResponse<MediaPageView>, ErrorDto> {
    if input.include_adult {
        return Err(storage_error(
            mediaatlas_infrastructure::library::RepositoryError::ProtectedMetadata,
        ));
    }
    input.include_adult = false;
    state
        .browse
        .clone()
        .for_tools()
        .list_media(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn library_tools_detail(
    state: State<'_, DesktopState>,
    input: ToolsMediaInput,
) -> Result<ApiResponse<MediaDetailView>, ErrorDto> {
    state
        .browse
        .clone()
        .for_tools()
        .get_media(&MediaDetailInput {
            media_id: input.media_id,
            include_adult: false,
        })
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn adult_organization_preview(
    state: State<'_, DesktopState>,
    input: OrganizationRequest,
) -> Result<ApiResponse<OrganizationPlan>, ErrorDto> {
    let _ = &input;
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let service = OrganizationService::new_adult(state.repository.clone(), access.clone());
    let result = service.preview(input).await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_organization_execute(
    state: State<'_, DesktopState>,
    input: OperationIdInput,
) -> Result<ApiResponse<OrganizationResult>, ErrorDto> {
    let _ = &input;
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let service = OrganizationService::new_adult(state.repository.clone(), access.clone());
    let result = service.execute(input.id).await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_organization_cancel(
    state: State<'_, DesktopState>,
    input: OperationIdInput,
) -> Result<ApiResponse<OrganizationResult>, ErrorDto> {
    let _ = &input;
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let service = OrganizationService::new_adult(state.repository.clone(), access.clone());
    let result = service.cancel(input.id).await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_organization_rollback_preview(
    state: State<'_, DesktopState>,
    input: OperationIdInput,
) -> Result<ApiResponse<OrganizationPlan>, ErrorDto> {
    let _ = &input;
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let service = OrganizationService::new_adult(state.repository.clone(), access.clone());
    let result = service
        .rollback_preview(input.id)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_organization_rollback_execute(
    state: State<'_, DesktopState>,
    input: OperationIdInput,
) -> Result<ApiResponse<OrganizationResult>, ErrorDto> {
    let _ = &input;
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let service = OrganizationService::new_adult(state.repository.clone(), access.clone());
    let result = service
        .rollback_execute(input.id)
        .await
        .map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_organization_discard(
    state: State<'_, DesktopState>,
    input: OperationIdInput,
) -> Result<ApiResponse<()>, ErrorDto> {
    let _ = &input;
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let service = OrganizationService::new_adult(state.repository.clone(), access.clone());
    service.discard(input.id).await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(()))
}

#[tauri::command]
pub async fn adult_organization_history(
    state: State<'_, DesktopState>,
    input: ReviewEmptyInput,
) -> Result<ApiResponse<Vec<OrganizationResult>>, ErrorDto> {
    let _ = &input;
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let service = OrganizationService::new_adult(state.repository.clone(), access.clone());
    let result = service.history().await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_open(
    state: State<'_, DesktopState>,
    input: OpenFileInput,
) -> Result<ApiResponse<VideoOpenResult>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let service = VideoOpenService::new_adult(state.repository.clone(), access);
    service
        .open(input.file_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn adult_tools_media(
    state: State<'_, DesktopState>,
    mut input: BrowseQuery,
) -> Result<ApiResponse<MediaPageView>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    mediaatlas_infrastructure::library_ready::backfill_adult_local_nfo(
        &state.repository,
        access.clone(),
    )
    .await
    .map_err(storage_error)?;
    input.include_adult = true;
    input.kind = Some(MediaKind::AdultVideo);
    let covers = state
        .adult_privacy
        .settings()
        .await
        .map_err(storage_error)?
        .covers_visible;
    let browse = state
        .adult_browse
        .clone()
        .with_adult_access(access.clone(), covers)
        .for_tools();
    let result = browse.list_media(&input).await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_tools_detail(
    state: State<'_, DesktopState>,
    input: ToolsMediaInput,
) -> Result<ApiResponse<MediaDetailView>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    mediaatlas_infrastructure::library_ready::backfill_adult_local_nfo(
        &state.repository,
        access.clone(),
    )
    .await
    .map_err(storage_error)?;
    let covers = state
        .adult_privacy
        .settings()
        .await
        .map_err(storage_error)?
        .covers_visible;
    let browse = state
        .adult_browse
        .clone()
        .with_adult_access(access.clone(), covers)
        .for_tools();
    let result = browse
        .get_media(&MediaDetailInput {
            media_id: input.media_id,
            include_adult: true,
        })
        .await
        .map_err(storage_error)?;
    if result.media.kind != MediaKind::AdultVideo {
        return Err(storage_error(
            mediaatlas_infrastructure::library::RepositoryError::ProtectedMetadata,
        ));
    }
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn watcher_list(
    state: State<'_, DesktopState>,
    input: ReviewEmptyInput,
) -> Result<ApiResponse<Vec<WatcherStatus>>, ErrorDto> {
    let _ = input;
    state
        .watcher
        .list()
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn watcher_configure(
    state: State<'_, DesktopState>,
    input: WatcherInput,
) -> Result<ApiResponse<WatcherStatus>, ErrorDto> {
    state
        .watcher
        .configure(input.root_id, input.enabled)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn health_check(
    state: State<'_, DesktopState>,
    input: HealthQuery,
) -> Result<ApiResponse<HealthPage>, ErrorDto> {
    state
        .health
        .check(&input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}

#[tauri::command]
pub async fn organization_plan(
    state: State<'_, DesktopState>,
    input: OperationIdInput,
) -> Result<ApiResponse<OrganizationPlan>, ErrorDto> {
    let service = state.organization.clone();
    let result = service.plan(input.id).await.map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn organization_logs(
    state: State<'_, DesktopState>,
    input: OperationIdInput,
) -> Result<ApiResponse<Vec<OrganizationLog>>, ErrorDto> {
    let service = state.organization.clone();
    let result = service.logs(input.id).await.map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_organization_plan(
    state: State<'_, DesktopState>,
    input: OperationIdInput,
) -> Result<ApiResponse<OrganizationPlan>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let service = OrganizationService::new_adult(state.repository.clone(), access.clone());
    let result = service.plan(input.id).await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}

#[tauri::command]
pub async fn adult_organization_logs(
    state: State<'_, DesktopState>,
    input: OperationIdInput,
) -> Result<ApiResponse<Vec<OrganizationLog>>, ErrorDto> {
    let access = state.adult_privacy.access().await.map_err(storage_error)?;
    let service = OrganizationService::new_adult(state.repository.clone(), access.clone());
    let result = service.logs(input.id).await.map_err(storage_error)?;
    access.ensure().map_err(storage_error)?;
    Ok(ApiResponse::new(result))
}
