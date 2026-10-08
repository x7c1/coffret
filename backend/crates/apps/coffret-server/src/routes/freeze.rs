use std::sync::Arc;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use coffret_device::FreezePreview;
use serde::Serialize;

use crate::api_error::ApiError;
use crate::entry_query::PathQuery;
use crate::folder::Folder;
use crate::freeze::{freeze_folder, packs_already, Book};
use crate::state::ServerState;

use super::work::WorkDto;

/// `POST /api/freeze?path=<folder>`
///
/// Packs a folder's eligible files into Packs: every file under it, at every
/// depth, that a freeze selects — a file not yet in the Library, and one whose
/// Entry a one-file Container holds (spec: PK-1, PK-17). It is what the
/// explorer's "Pack this folder…" arms, once the person has been shown what
/// [`GET /api/freeze`](preview) counted and has said yes. Entries a Pack
/// already holds are left as they are (spec: PK-2), and a file this device
/// never placed is not this device's to pack (spec: EP-10).
///
/// A retry of a stopped drop is the same call. A drop added as a Pack arms its
/// freeze with the files it wrote as the selection, and this server keeps that
/// selection beside the run; where one is kept for the folder named, this packs
/// exactly that selection again rather than the whole folder, so a one-file
/// Entry the folder also holds is not drawn into a drop's retry (spec: PK-17).
/// Where nothing is kept it is the whole folder — and that is the one shape the
/// Library root cannot take, since a freeze narrowed to nothing would pack the
/// whole Library.
///
/// It takes the folder as `?path=`, the spelling every route here names a place
/// in the Library with, for the reason
/// [`PathQuery`](crate::entry_query::PathQuery) gives — the folder the work
/// answer names the run by.
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
    let book = armable(&state, query).await?;
    freeze_folder(Arc::clone(&state), book);
    Ok((StatusCode::ACCEPTED, Json(WorkDto::of(&state))))
}

/// `GET /api/freeze?path=<folder>`
///
/// What [`POST /api/freeze`](freeze) of the same folder would pack on this
/// device, counted before it is asked for: how many files and how many bytes,
/// at every depth under the folder — and how many it would leave out, in three
/// broad groups, so that a count smaller than the folder is not a surprise.
///
/// The count is the freeze's own scan, stopped before the first file is read
/// (see [`preview_freeze`](coffret_device::OpenLibrary::preview_freeze)), over
/// the same book the `POST` would arm — a kept selection included — so the two
/// cannot disagree about which files are selected. They can disagree only
/// where the folder or the catalog changes between the two calls.
///
/// It changes nothing. No Storage is reached, no key is used beyond holding the
/// Library open, nothing is written on this device, and nothing is armed. It is
/// refused exactly where the `POST` is — a folder no mapping of this device
/// reaches, and the Library root with no selection kept for it — because a
/// count of what a refused call would pack is a count of nothing.
pub async fn preview(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<PathQuery>,
) -> Result<Json<PreviewDto>, ApiError> {
    let book = armable(&state, query).await?;
    let preview = state
        .unlocked()?
        .preview_freeze(book.folder.listed(), book.only.as_ref())
        .await?;
    Ok(Json(PreviewDto::of(
        &book,
        &preview,
        state.freezes.running(),
        packs_already(&state.freezes, &book),
    )))
}

/// The book a freeze of the folder `query` names would pack, or the refusal
/// that keeps one from being armed.
async fn armable(state: &ServerState, query: PathQuery) -> Result<Book, ApiError> {
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
    Ok(book)
}

/// What a freeze of one folder would pack, as the explorer asks before it
/// offers the folder's Pack button.
#[derive(Debug, Serialize)]
pub struct PreviewDto {
    /// The folder the freeze would be named by.
    folder: String,
    /// How many files it would pack.
    files: usize,
    /// How many bytes those files come to on this device.
    bytes: u64,
    /// How many files under the folder a Pack already holds (spec: PK-2).
    in_pack: usize,
    /// How many Pack-held files have moved on this device since it last saw
    /// them, which a freeze leaves to `update` (spec: PK-2, PK-14).
    changed_in_pack: usize,
    /// How many Entries under the folder this device has no file of its own
    /// for (spec: EP-10).
    not_here: usize,
    /// How many of the mappings the folder lies under this device cannot
    /// reach right now — at most one, for a folder — under which nothing was
    /// counted, not even as not here (spec: EP-12).
    unavailable: usize,
    /// Whether a freeze is already running, so this one would wait its turn.
    after_current: bool,
    /// Whether this folder is being packed already, so the `POST` would arm
    /// no run of its own: the run under way packs every file this one asks
    /// for, or the folder is waiting its turn (see
    /// [`packs_already`](crate::freeze::packs_already)).
    already_packing: bool,
}

impl PreviewDto {
    fn of(
        book: &Book,
        preview: &FreezePreview,
        after_current: bool,
        already_packing: bool,
    ) -> Self {
        Self {
            folder: book.folder.as_str().to_owned(),
            files: preview.files,
            bytes: preview.bytes,
            in_pack: preview.in_pack,
            changed_in_pack: preview.changed_in_pack,
            not_here: preview.not_here,
            unavailable: preview.unavailable.len(),
            after_current,
            already_packing,
        }
    }
}
