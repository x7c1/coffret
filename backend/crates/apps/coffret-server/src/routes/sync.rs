use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use crate::state::ServerState;
use crate::sync::arm_sync;

use super::activity::ActivityDto;

/// `POST /api/sync`
///
/// Carries the mapped folders into the Library again.
///
/// This is not a "sync now" button and there is deliberately not one. What syncs
/// a dropped file is dropping it: the upload arms this itself, and the person
/// who added the file has already said everything there is to say. It exists for
/// what that trigger cannot express — a sync Storage stopped, whose files are
/// sitting in the folder with nothing left to drop — where the alternative is
/// telling somebody to add a file they have already added.
///
/// It takes no path. Which folders a sync walks is the device's mappings
/// (spec: EP-9) and never an argument, here as on the command line: a route that
/// narrowed it would be a second reading of what a sync covers.
///
/// It answers with the activity as it stands the moment the sync is armed, rather
/// than waiting for it: the work runs in the background and the browser polls for
/// the rest of it. `202` says exactly that.
pub async fn sync(State(state): State<Arc<ServerState>>) -> (StatusCode, Json<ActivityDto>) {
    arm_sync(Arc::clone(&state));
    (
        StatusCode::ACCEPTED,
        Json(ActivityDto::of(
            state.fills.activity(),
            state.syncs.activity(),
        )),
    )
}
