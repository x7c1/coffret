use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use coffret_device::RemoteHead;
use serde::Serialize;

use crate::api_error::ApiError;
use crate::state::ServerState;

/// Whether the Library has a head this device's catalog has not caught up with.
#[derive(Serialize)]
pub struct RemoteHeadDto {
    head: HeadDto,
}

/// The two answers, in the wire's words.
///
/// No generations: what a screen does with this is draw the control that
/// catches up as waiting or not, and the generations are the Library's own
/// bookkeeping (spec: CK-1), which the catch-up's line records once it runs.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum HeadDto {
    /// Another device has committed since this one last caught up.
    Newer,
    /// There is nothing newer than the catalog.
    Current,
}

impl From<RemoteHead> for HeadDto {
    fn from(head: RemoteHead) -> Self {
        match head {
            RemoteHead::Newer => Self::Newer,
            RemoteHead::Current => Self::Current,
        }
    }
}

/// `GET /api/remote-head`
///
/// Asks Storage which heads the Library has, by the names its listing gives,
/// and compares the newest with the one this device's catalog stands at. The
/// explorer asks it when it loads, when its tab comes back to the front, and
/// every few minutes while the tab is shown, so that the control that catches
/// up can say there is something to catch up with.
///
/// It is a look and nothing more. No object is read — no Journal record, no
/// Keyring replica, no Container — so no key is reached for and nothing the
/// provider holds is opened; the catalog is not moved, which stays the refresh's
/// to do when somebody presses it (spec: CK-9); and it does not wait for the
/// turn a catch-up takes, since it replays nothing two callers could replay
/// twice. Nor does it count as somebody being here: an open tab asking on a
/// timer is not a person, and the idle lock is not put off by it (spec: DK-4).
///
/// While the Library is locked it is refused as every route that reaches the
/// Library is, because the Storage it asks goes with the open Library.
pub async fn remote_head(
    State(state): State<Arc<ServerState>>,
) -> Result<Json<RemoteHeadDto>, ApiError> {
    let library = state.unlocked_unattended()?;
    let head = library.remote_head().await?;
    Ok(Json(RemoteHeadDto { head: head.into() }))
}
