use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::state::ServerState;

/// Which Library the browser is looking at.
///
/// Three fields, and what is missing from them is the point. Not the epoch, not
/// the checkpoint, not the folder id or the bucket name: a page has no use for
/// any of it, and each would be one more thing the browser holds about where the
/// user's data lives. The provider is here because "on Google Drive" is
/// something a person recognizes their own Library by; it names the provider and
/// nothing about the account.
#[derive(Serialize)]
pub struct LibraryDto {
    name: String,
    library_id: String,
    provider: &'static str,
}

/// `GET /api/library`
///
/// One of the routes a locked server still answers, and for the reason these
/// three fields are the ones it has: which Library this is, what it is
/// called here, and which provider it is on are not things the Master Key
/// keeps — they are read off the settings file that any process on this device
/// can open. A status bar that went blank the moment the server locked would
/// leave a person looking at a screen with no name on it, unable to tell which
/// of their devices had gone quiet.
pub async fn library(State(state): State<Arc<ServerState>>) -> Json<LibraryDto> {
    Json(LibraryDto {
        name: state.name.clone(),
        library_id: state.library_id().to_owned(),
        provider: state.provider(),
    })
}
