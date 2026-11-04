//! Queue bookkeeping never runs a merge, source output or restart replay.
use super::{storage_error, DesktopState};
use mediaatlas_application::{library::ApiResponse, review_queue::*};
use mediaatlas_desktop_bridge::ErrorDto;
use mediaatlas_infrastructure::review_queue::ReviewQueueService;
use tauri::State;

#[tauri::command]
pub async fn review_queue_create(
    state: State<'_, DesktopState>,
    input: ReviewQueueCreate,
) -> Result<ApiResponse<ReviewQueueView>, ErrorDto> {
    ReviewQueueService::new(state.repository.clone())
        .create(input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn review_queue_get(
    state: State<'_, DesktopState>,
    input: ReviewQueueId,
) -> Result<ApiResponse<ReviewQueueView>, ErrorDto> {
    ReviewQueueService::new(state.repository.clone())
        .get(input.queue_id)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn review_queue_list(
    state: State<'_, DesktopState>,
    input: ReviewQueueList,
) -> Result<ApiResponse<Vec<ReviewQueueView>>, ErrorDto> {
    let _ = input;
    ReviewQueueService::new(state.repository.clone())
        .list()
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn review_queue_update(
    state: State<'_, DesktopState>,
    input: ReviewQueueUpdate,
) -> Result<ApiResponse<ReviewQueueView>, ErrorDto> {
    ReviewQueueService::new(state.repository.clone())
        .update(input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
#[tauri::command]
pub async fn review_queue_checkpoint(
    state: State<'_, DesktopState>,
    input: ReviewQueueCheckpoint,
) -> Result<ApiResponse<ReviewQueueView>, ErrorDto> {
    ReviewQueueService::new(state.repository.clone())
        .checkpoint(input)
        .await
        .map(ApiResponse::new)
        .map_err(storage_error)
}
