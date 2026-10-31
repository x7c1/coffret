use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;

use crate::api_error::ApiError;
use crate::state::ServerState;
use crate::sync::arm_sync;

use super::work::WorkDto;

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
/// It takes no path, and the sync it arms walks every mapped folder
/// (spec: EP-9, EP-10), as the command line's does: what it retries may be any
/// drop's, and a file waiting in a folder no drop's sync walked is carried in
/// by this. A sync a drop armed covers that drop's folders alone; one armed
/// here while such a sync waits for its turn is taken into it, and covers
/// both.
///
/// It answers with the work answer as it stands the moment the sync is armed, rather
/// than waiting for it: the work runs in the background and the browser polls for
/// the rest of it. `202` says exactly that.
pub async fn sync(
    State(state): State<Arc<ServerState>>,
) -> Result<(StatusCode, Json<WorkDto>), ApiError> {
    // Asked and thrown away, exactly as the fill's route asks and for the
    // reason it gives (spec: DK-2).
    state.unlocked()?;
    arm_sync(Arc::clone(&state), None);
    Ok((StatusCode::ACCEPTED, Json(WorkDto::of(&state).await)))
}
