use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;

use crate::api_error::ApiError;
use crate::entry_query::PathQuery;
use crate::fill::{fill_folder, Folder};
use crate::state::ServerState;

use super::activity::ActivityDto;

/// `POST /api/fill?path=<folder>`
///
/// Takes the folder up again.
///
/// This is not a download button and there is deliberately not one: what brings
/// a folder over is opening a file in it, and every path that leads here has
/// already done that. It exists for what the implicit trigger cannot express —
/// a fill Storage stopped, and a fill that was superseded when somebody clicked
/// away — where the alternative would be telling a person to open a file they
/// have already opened.
///
/// It answers with the activity as it stands the moment the fill is armed,
/// rather than waiting for it: the work runs in the background and the browser
/// polls for the rest of it. `202` says exactly that.
pub async fn fill(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<PathQuery>,
) -> Result<(StatusCode, Json<ActivityDto>), ApiError> {
    let folder = Folder::named(query.folder()?);
    fill_folder(Arc::clone(&state), folder);
    Ok((
        StatusCode::ACCEPTED,
        Json(ActivityDto::of(
            state.fills.activity(),
            state.syncs.activity(),
        )),
    ))
}
