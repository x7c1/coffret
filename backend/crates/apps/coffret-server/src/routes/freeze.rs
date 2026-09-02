use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;

use crate::api_error::ApiError;
use crate::entry_query::PathQuery;
use crate::folder::Folder;
use crate::freeze::freeze_folder;
use crate::state::ServerState;

use super::activity::ActivityDto;

/// `POST /api/freeze?path=<folder>`
///
/// Packs the folder into Packs again.
///
/// This is not a "pack this" button and there is deliberately not one. What
/// freezes a book is bringing it in — dropping its pages onto a folder made a
/// moment ago in the browser, which arms this itself — and the person who
/// dropped them has already said everything there is to say. It exists for what
/// that trigger cannot express: a freeze Storage stopped, whose pages are
/// sitting in the folder with nothing left to drop, where the alternative is
/// telling somebody to drop a book they have already dropped.
///
/// It takes the folder as `?path=`, the spelling every route here names a place
/// in the Library with, for the reason
/// [`PathQuery`](crate::entry_query::PathQuery) gives. Unlike a sync it has to
/// take one: a freeze is of one folder (spec: PK-17), and one narrowed to
/// nothing would be a book import that packed the whole Library.
///
/// A folder no mapping of this device reaches is refused before anything is
/// armed. There is nowhere under it for a local file to be (spec: EP-9), so the
/// run would walk to select nothing and commit nothing — and a `202` for work
/// that cannot happen is a browser told to follow a freeze that will never say
/// anything.
///
/// It answers with the activity as it stands the moment the freeze is armed,
/// rather than waiting for it: the work runs in the background and the browser
/// polls for the rest of it. `202` says exactly that. A second call while one is
/// running queues the folder behind it rather than starting a second run — one
/// book at a time — and a call naming the book already running or already
/// waiting changes nothing at all.
pub async fn freeze(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<PathQuery>,
) -> Result<(StatusCode, Json<ActivityDto>), ApiError> {
    // A `?path=` that is absent or empty is the Library root everywhere else on
    // these routes, and the root is the one place this route cannot take: a
    // freeze whose prefix is nothing selects every eligible Entry the mappings
    // reach (spec: PK-17), so a parameter left out would pack the whole Library
    // — the command line's own run, arrived at by omission, and one no drop can
    // ask for. A device that maps the Library root has nothing else standing
    // between the two.
    let Some(named) = query.folder()? else {
        return Err(ApiError::bad_path(
            "it names no folder, and a freeze is of one folder rather than of the whole Library",
        ));
    };
    let folder = Folder::named(Some(named));
    if !state.unlocked()?.list(folder.listed()).await?.mapped {
        return Err(ApiError::no_folder_here());
    }
    freeze_folder(Arc::clone(&state), folder);
    Ok((StatusCode::ACCEPTED, Json(ActivityDto::of(&state))))
}
