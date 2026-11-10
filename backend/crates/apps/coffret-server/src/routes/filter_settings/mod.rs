use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::Json;
use coffret_device::FilterSettings;

use crate::api_error::{ApiError, NotJoined};
use crate::state::ServerState;

/// `GET /api/filter-settings`
///
/// What this device's explorer last had its tree filtered by, as
/// `{"name": …, "recentOn": …, "recent": …}`, and no filter — an empty name,
/// Recent off, a count of 10 — where nothing was kept or what was kept cannot
/// be read (spec: LA-43).
///
/// Refused while the Library is locked, as every route that reads the catalog
/// is (spec: DK-2), although nothing here needs its key: unlike the
/// appearance, a name typed to filter the tree says something about what the
/// Library holds, so a locked server does not hand it out. Needing no key, it
/// records nobody being here (spec: DK-4).
pub async fn filter_settings(
    State(state): State<Arc<ServerState>>,
) -> Result<Json<FilterSettings>, ApiError> {
    state.unlocked_unattended()?;
    let file = state.explorer.clone();
    let settings = tokio::task::spawn_blocking(move || file.filter_settings())
        .await
        .map_err(|cause| ApiError::server(NotJoined("FilterSettingsNotJoined", cause)))?;
    Ok(Json(settings))
}

/// `POST /api/filter-settings` with `{"name": …, "recentOn": …, "recent": …}`
///
/// Keeps the settings for this device, replacing the ones before and leaving
/// the kept appearance as it was, and answers with them as `GET` would (spec:
/// LA-43). A body that is not settings the explorer could have been set to —
/// a count that is not a whole number from 1 to 4,294,967,295 (`u32::MAX`),
/// a name longer than
/// [`MOST_FILTER_NAME_CHARS`](coffret_device::MOST_FILTER_NAME_CHARS)
/// characters, a field missing or one more — is refused as `400` before
/// anything is written. Refused while locked for the reason the `GET` is.
pub async fn keep_filter_settings(
    State(state): State<Arc<ServerState>>,
    body: Result<Json<FilterSettings>, JsonRejection>,
) -> Result<Json<FilterSettings>, ApiError> {
    state.unlocked_unattended()?;
    let Json(settings) = body.map_err(ApiError::unreadable_json)?;
    let file = state.explorer.clone();
    let kept = settings.clone();
    tokio::task::spawn_blocking(move || file.keep_filter_settings(&kept))
        .await
        .map_err(|cause| ApiError::server(NotJoined("FilterSettingsNotJoined", cause)))??;
    Ok(Json(settings))
}
