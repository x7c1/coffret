use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use coffret_device::EntryPath;
use serde::Serialize;

use crate::api_error::ApiError;
use crate::state::ServerState;

/// Every folder in the Library, flat and sorted.
///
/// Flat because a Library has no folders to nest (spec: EP-2): what comes back
/// is every path a separator implies, each named in full, and the tree the
/// explorer draws is the browser's arrangement of them. Sending a tree instead
/// would be sending one arrangement and calling it the Library.
#[derive(Serialize)]
pub struct FoldersDto {
    folders: Vec<String>,
}

/// `GET /api/folders`
pub async fn folders(State(state): State<Arc<ServerState>>) -> Result<Json<FoldersDto>, ApiError> {
    let folders = state.unlocked()?.folders().await?;
    Ok(Json(FoldersDto {
        folders: folders
            .iter()
            .map(EntryPath::as_str)
            .map(str::to_owned)
            .collect(),
    }))
}
