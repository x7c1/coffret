use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;

use crate::api_error::ApiError;
use crate::entry_query::PathQuery;
use crate::folder::Folder;
use crate::freeze::freeze_folder;
use crate::state::ServerState;

use super::work::WorkDto;

/// `POST /api/freeze?path=<folder>`
///
/// Packs the folder into Packs again.
///
/// This is not a "pack this" button and there is deliberately not one. What
/// freezes a book is bringing it in — dropping a folder and choosing to add it
/// as a Pack, which arms this itself — and the person who dropped it has
/// already said everything there is to say. It exists for what that trigger
/// cannot express: a freeze Storage stopped, whose pages are sitting in the
/// folder with nothing left to drop, where the alternative is telling somebody
/// to drop a book they have already dropped.
///
/// It takes the folder as `?path=`, the spelling every route here names a place
/// in the Library with, for the reason
/// [`PathQuery`](crate::entry_query::PathQuery) gives — the folder the work
/// answer named the run by. What it packs again is what that run was asked to
/// pack: the files its drop wrote, which this server kept beside the run, so a
/// one-file Entry the folder also holds is not drawn in (spec: PK-17). Where
/// nothing is kept for the folder it is every file under it, as a retry always
/// was — and that is the one shape the Library root cannot take, since a
/// freeze narrowed to nothing would be a book import that packed the whole
/// Library.
///
/// A folder no mapping of this device reaches is refused before anything is
/// armed. There is nowhere under it for a local file to be (spec: EP-9), so the
/// run would walk to select nothing and commit nothing — and a `202` for work
/// that cannot happen is a browser told to follow a freeze that will never say
/// anything.
///
/// It answers with the work answer as it stands the moment the freeze is armed,
/// rather than waiting for it: the work runs in the background and the browser
/// polls for the rest of it. `202` says exactly that. A second call while one is
/// running queues the folder behind it rather than starting a second run — one
/// book at a time — and a call naming the book already running or already
/// waiting changes nothing at all.
pub async fn freeze(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<PathQuery>,
) -> Result<(StatusCode, Json<WorkDto>), ApiError> {
    // A `?path=` that is absent or empty is the Library root everywhere else on
    // these routes. A run kept for the root is a drop onto it, and its
    // selection bounds it; with nothing kept, a freeze whose prefix is nothing
    // selects every eligible Entry the mappings reach (spec: PK-17), so a
    // parameter left out would pack the whole Library — the command line's own
    // run, arrived at by omission, and one no drop can ask for. A device that
    // maps the Library root has nothing else standing between the two.
    let book = state.freezes.again(Folder::named(query.folder()?));
    if book.is_whole_library() {
        return Err(ApiError::bad_path(
            "it names no folder, and a freeze is of one folder rather than of the whole Library",
        ));
    }
    if !state.unlocked()?.list(book.folder.listed()).await?.mapped {
        return Err(ApiError::no_folder_here());
    }
    freeze_folder(Arc::clone(&state), book);
    Ok((StatusCode::ACCEPTED, Json(WorkDto::of(&state))))
}
