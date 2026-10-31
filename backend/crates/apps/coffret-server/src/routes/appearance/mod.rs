use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::Json;
use coffret_device::Appearance;
use serde::Serialize;

use crate::api_error::{ApiError, NotJoined};
use crate::state::ServerState;

mod appearance_request;
pub use appearance_request::AppearanceRequest;

/// Which set of colours this device's explorer is drawn in: `system`, `light`
/// or `dark`.
#[derive(Serialize)]
pub struct AppearanceDto {
    appearance: Appearance,
}

/// `GET /api/appearance`
///
/// The set of colours this device's explorer is drawn in, as it was last
/// chosen here, and `system` where nothing was chosen or what was kept cannot
/// be read (spec: LA-38).
///
/// One of the routes a locked server still answers: it is the device's
/// preference about how a page looks, kept in no Library and under no key.
/// (A page reaches it through the explorer host, whose fence refuses it while
/// the Library is locked all the same: spec: LA-14.) It goes
/// through no door that records somebody being here, since a page asking how
/// to draw itself is not a person wanting the Library (spec: DK-4).
pub async fn appearance(
    State(state): State<Arc<ServerState>>,
) -> Result<Json<AppearanceDto>, ApiError> {
    let file = state.appearance.clone();
    let appearance = tokio::task::spawn_blocking(move || file.read())
        .await
        .map_err(|cause| ApiError::server(NotJoined("AppearanceNotJoined", cause)))?;
    Ok(Json(AppearanceDto { appearance }))
}

/// `POST /api/appearance` with `{ "appearance": "system" | "light" | "dark" }`
///
/// Keeps the choice for this device, replacing the one before, and answers with
/// it as `GET` would (spec: LA-38). Answered while locked for the reason the
/// `GET` is, and recording nobody being here for the same reason.
pub async fn choose_appearance(
    State(state): State<Arc<ServerState>>,
    body: Result<Json<AppearanceRequest>, JsonRejection>,
) -> Result<Json<AppearanceDto>, ApiError> {
    let Json(request) = body.map_err(ApiError::unreadable_json)?;
    let file = state.appearance.clone();
    let appearance = request.appearance;
    tokio::task::spawn_blocking(move || file.write(appearance))
        .await
        .map_err(|cause| ApiError::server(NotJoined("AppearanceNotJoined", cause)))??;
    Ok(Json(AppearanceDto { appearance }))
}
