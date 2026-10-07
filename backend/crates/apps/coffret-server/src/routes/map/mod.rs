use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::Json;
use coffret_device::{MarkerRecord, MarkerRequest};
use serde::Serialize;

use super::browse::absolute;
use crate::api_error::ApiError;
use crate::state::ServerState;

mod map_request;
pub use map_request::MapRequest;

/// What recording a mapping did — what `coffret map` says, as fields.
#[derive(Serialize)]
pub struct MappedDto {
    /// The top-level folder the mapping is for, and `null` for the Library
    /// root.
    prefix: Option<String>,
    /// The folder it now points at, as it was recorded: a whole path with no
    /// symbolic link left in it.
    local_root: String,
    /// The folder the same prefix pointed at before, where it was already
    /// mapped.
    ///
    /// Said rather than dropped, because moving a mapping takes everything
    /// under the old folder out of the Library's reach on this device, and a
    /// person who chose the wrong folder has no other way of noticing.
    replaced: Option<String>,
    /// What became of the folder's identity (spec: EP-13): `written` where it
    /// carried none, `adopted` where it already carried one, which was kept.
    marker: &'static str,
    /// One sentence a person could read.
    message: String,
}

/// `POST /api/map` with `{ "local_root": "<absolute>", "prefix": … }`
///
/// Records that a folder on this device holds the Library root or one of its
/// top-level folders (spec: EP-9) — the explorer's `coffret map`, through the
/// same device-crate body, and so with the same refusals in the same order. The
/// folder's identity is adopted where it already carries one and written where
/// it carries none (spec: EP-13); asking for a new one is the command line's
/// `--reset-marker`, which has no counterpart here, because the one situation
/// that calls for it — a copy that has to be told apart from what it was copied
/// from — is not one a person should meet by choosing a folder from a list.
///
/// Keyed like every route (spec: LA-2), and refused while the Library is
/// locked. A mapping needs no key — it is the device's record, not the
/// Library's data (spec: CK-7) — and the command line records one without a
/// Passphrase. What it takes from the open Library is the catalog this server
/// already reads its mappings through, and that is the reason for the lock:
/// a mapping recorded through it is the mapping the very next listing reads,
/// and on a locked server there is no listing for a page to have offered the
/// gesture from. Mappings are read per request, as they are for one the
/// command line records while this server runs, so nothing is restarted and no
/// answer is held over.
pub async fn map(
    State(state): State<Arc<ServerState>>,
    body: Result<Json<MapRequest>, JsonRejection>,
) -> Result<Json<MappedDto>, ApiError> {
    let library = state.unlocked()?;
    let Json(request) = body.map_err(|rejection| ApiError::unreadable_json(&rejection))?;
    let local_root = absolute(&request.local_root)?;

    let recorded = library
        .set_mapping(
            request.prefix.as_deref(),
            &local_root,
            MarkerRequest::AdoptWhatIsThere,
        )
        .await
        .map_err(ApiError::mapping_refused)?;

    let now = recorded.local_root.display().to_string();
    let replaced = recorded
        .replaced
        .as_ref()
        .map(|mapping| mapping.local_root.display().to_string());
    let what = match &request.prefix {
        Some(prefix) => prefix.clone(),
        None => "the Library root".to_owned(),
    };
    let message = match &replaced {
        Some(before) => format!("{what} was at {before}; it is now at {now}"),
        None => format!("{what} is at {now}"),
    };
    Ok(Json(MappedDto {
        prefix: request.prefix,
        local_root: now,
        replaced,
        marker: match recorded.marker {
            MarkerRecord::Written => "written",
            MarkerRecord::Adopted => "adopted",
            MarkerRecord::Reset => "reset",
        },
        message,
    }))
}
