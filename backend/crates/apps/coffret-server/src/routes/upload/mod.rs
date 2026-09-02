//! The one route that carries anything into the Library.
//!
//! The route itself is here with the shapes it is asked and answered with;
//! taking one part of a drop is `receive`, and where a refusal about one file
//! and a refusal about the whole request part company is `refusal`.

use std::sync::Arc;

use axum::extract::{Multipart, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use tracing::info;

use crate::api_error::ApiError;
use crate::entry_query::folder_named;
use crate::folder::Folder;
use crate::freeze::freeze_folder;
use crate::reported::Reported;
use crate::state::ServerState;
use crate::sync::arm_sync;

use super::activity::RefusalDto;

use self::declared_length::declared_length;
use self::outran::outran;
use self::receive::receive;
use self::refusal::Refusal;
use self::refused_dto::RefusedDto;

mod declared_length;

mod landed;

mod outran;

mod receive;

mod refusal;

mod refused_dto;

mod room_for;

mod under;

mod upload_dto;
pub use upload_dto::UploadDto;

mod upload_query;
pub use upload_query::UploadQuery;

/// `POST /api/upload?path=<folder>[&freeze=true]`, multipart
///
/// Adds files to one folder of the Library.
///
/// The folder is `?path=`, the spelling every route here names a place in the
/// Library with, for the reason
/// [`PathQuery`](crate::entry_query::PathQuery) gives — a second name for the
/// same parameter would be a second thing to keep in step.
///
/// Each part is one file, and its filename is the path *relative to the folder* —
/// `photo.jpg` for a file dropped on its own, `holiday/day1/photo.jpg` for one
/// inside a dropped folder — so one route serves both shapes and the folders a
/// person dropped are made on the way. Where the part lands is `<folder>` and
/// that relative path, shaped and normalized like every other path these routes
/// take (spec: EP-1, EP-2).
///
/// # What it does, and what it deliberately is not
///
/// It writes files into the folder this device maps that part of the Library into
/// (spec: EP-9) and arms the work that carries them in. That is the whole of it:
/// adding a file to a Library has always meant putting it in a mapped folder and
/// letting a flow carry it in, and this is that gesture performed for somebody
/// who is in a browser rather than a file manager. Nothing here encrypts,
/// uploads or commits anything, and no part of the Library changes until that
/// flow commits.
///
/// # Which flow, and why the browser says
///
/// A plain drop arms a sync, which is the same gesture as copying the files in
/// and typing `coffret sync`: one Container per file, which is the right shape
/// for the handful of files a drop usually is.
///
/// `?freeze=true` arms a freeze of the folder instead (spec: PK-17), and is what
/// the explorer sends for a drop onto a folder the person made in the browser a
/// moment ago. That is a book being brought in — a folder of a few hundred page
/// images arriving in one gesture — and a sync would make it a few hundred
/// Storage Objects, a few hundred uploads, and a few hundred provider calls to
/// open again. The freeze packs them instead, so they go up once, as Packs.
///
/// Which of the two it is comes from the caller and is not worked out here, for
/// the reason [`UploadQuery`] gives: from the server the two drops look
/// identical.
///
/// A book drop names its folder. `?freeze=true` with no `?path=` is refused
/// before the parts are read: the freeze's prefix is the folder, and one
/// narrowed to nothing packs everything the mappings reach (spec: PK-17) rather
/// than the pages that were dropped.
///
/// # Refused before anything lands
///
/// Three refusals, and every one of them is settled before a byte reaches disk.
/// A folder no mapping of this device reaches takes the whole drop with it: there
/// is nowhere to put any of it (spec: EP-9), and the listing has already said so
/// over the rows. A part whose relative path is not an Entry Path is refused by
/// name (spec: EP-2). And a part standing where the Library holds an Entry inside
/// a Pack is refused by name too, because coffret cannot yet replace one
/// (spec: PK-10, PK-12) — writing it would leave a file in the folder that no
/// flow will ever carry in. An Entry in a Container of its own (spec: PK-15) is
/// not refused: a changed mapped file is eligible for `update`, and replacing
/// the one Container holding it is ordinary work (spec: PK-11, PK-12).
///
/// A part that is refused still has its bytes read off the wire and dropped. The
/// alternative is answering in the middle of a request the browser is still
/// sending, which no browser reads.
///
/// # Refused because of what it would cost
///
/// Those three are about the Library. Three more are about this server and this
/// device, and they are the three budgets [`Envelope`](crate::Envelope) states:
/// how much one request may carry, how much one part of it may, and how many
/// parts there may be. Passing one of them is not a part being refused — it
/// stops the request where it stands, because a request that has already passed
/// a budget is one whose remaining bytes there is no reason to read.
///
/// So this is the one place the route does what it will not do for a refused
/// part: answer in the middle of a request the browser is still sending, and
/// pay for it — what reaches the person may be a transfer that failed rather
/// than the sentence it was answered with. What makes that worth paying here
/// and not for a refused part is the size of the other side. Reading a refused
/// part to the end costs what that one part costs and keeps the rest of the
/// drop going; reading out a request that has already passed a budget is doing
/// the whole of the thing the budget is there to refuse. So the sentence is
/// written for whoever does read it, and every one of these refusals is put in
/// the log as well — that is the half that always arrives.
///
/// Beside them is a question rather than a budget: whether the volume this
/// device's folder is on still has room for what is coming. It is asked of each
/// part before that part is taken, so a drop that would fill the disk is refused
/// while refusing is still cheap.
///
/// # One drop at a time, and why nothing here enforces it
///
/// The explorer drops one book and waits for the answer, and nothing on this
/// route serialises anything. That is deliberate rather than missing. Two drops
/// arriving at once write into different files and refuse each other nothing —
/// each part goes to a scratch name of its own and is renamed onto its
/// destination (spec: EP-11) — and what they arm behind them is already ordered
/// where ordering matters: a second book armed while one is being packed waits
/// its turn rather than displacing it, which is [`freeze_folder`]'s doing — a
/// freeze commits one batch (spec: PK-7), so a book put aside half way is one
/// that was never brought in at all — and a second sync collapses into the one
/// walk of the mappings that would have found both drops anyway. A queue in
/// front of this route would add nothing to either and would make a person
/// dropping a photograph wait behind somebody's book.
pub async fn upload(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<UploadQuery>,
    headers: HeaderMap,
    mut parts: Multipart,
) -> Result<Json<UploadDto>, ApiError> {
    // Before a single part is read, beside the three refusals below and for the
    // same reason: a drop that lands its files and then finds nothing can carry
    // them in is the one state a person must not be put in silently
    // (spec: DK-2).
    let library = state.unlocked()?;
    let folder = folder_named(query.path.as_deref())?;
    // A book goes into the folder made for it, and the Library root is not one.
    // A freeze whose prefix is nothing selects every eligible Entry the mappings
    // reach (spec: PK-17), so `?freeze=true` with no folder named would pack the
    // whole Library rather than the pages just dropped — and on a device that
    // maps the Library root nothing else would stop it. Refused here, before the
    // parts are read, for the reason the unmapped refusal below is.
    if query.freeze && folder.is_none() {
        return Err(ApiError::bad_path(
            "it names no folder, and a book is brought into a folder made for it rather than \
             into the Library root",
        ));
    }
    // Asked once, of the folder, rather than once per part: the mappings partition
    // the Library by top-level component (spec: EP-9), and every part of a drop
    // onto a folder carries that folder's component — so a folder a mapping
    // reaches leaves no part of the drop unreachable. Only at the Library root do
    // the parts carry components of their own, and there what is asked after is a
    // root mapping, which stands for every component no other mapping claims.
    if !library.list(folder.as_ref()).await?.mapped {
        return Err(ApiError::no_folder_here());
    }

    // What the request says it is bringing, where it says anything. Every browser
    // sending a `FormData` does; a caller that streams without saying is not
    // refused for it, and the fence below asks for the room one part could take
    // instead of the room the rest of the request will.
    let declared = declared_length(&headers);

    let mut written = Vec::new();
    let mut refused = Vec::new();
    let mut bytes = 0u64;
    let mut seen = 0usize;
    while let Some(part) = parts.next_field().await.map_err(ApiError::multipart)? {
        // Counted before it is looked at, and every part counts — one with no
        // filename included. What this budget is about is the request, and a
        // request made of a million parts this route would skip is still a
        // million parts to read.
        seen += 1;
        if seen > state.envelope.parts {
            return Err(outran(
                "one drop is one gesture, and this carries more files than one gesture \
                 takes — the same files in two drops are taken",
            ));
        }
        // A part with no filename is not a file. Nothing this route serves sends
        // one, and inventing a name for it would be inventing an Entry Path.
        let Some(name) = part.file_name().map(str::to_owned) else {
            continue;
        };
        // What is still to come, for the room this device is asked to have: what
        // the request declared less what has landed, and one part's worth where
        // it declared nothing.
        let coming = declared.map_or(state.envelope.part_bytes, |all| all.saturating_sub(bytes));
        match receive(
            &library,
            &state.envelope,
            coming,
            folder.as_ref(),
            &name,
            part,
        )
        .await
        {
            Ok(landed) => {
                bytes += landed.bytes;
                written.push(landed.path.as_str().to_owned());
            }
            Err(Refusal::Part(refusal)) => refused.push(RefusedDto {
                name,
                refusal: RefusalDto::of(&Reported::recorded(&refusal, "upload")),
            }),
            // Nothing is armed for what landed before it: this request was
            // refused, and arming its work would be answering a refusal with
            // the flow it asked for. Those files are in the folder and whole,
            // and they wait there as anything else copied into a mapped folder
            // waits — nothing on this server arms a sync on its own, so what
            // carries them in is a later drop that lands something, or somebody
            // asking for one.
            Err(Refusal::Request(refusal)) => return Err(refusal),
        }
    }

    // Only where something landed. A drop that was refused whole has left the
    // folder exactly as it was, and a run over an unchanged folder is a walk to
    // find nothing.
    if !written.is_empty() {
        match query.freeze {
            true => freeze_folder(Arc::clone(&state), Folder::named(folder.clone())),
            false => arm_sync(Arc::clone(&state)),
        }
    }

    // Counts and sizes, and which of the two flows was armed. No name of anything
    // reaches the event: the folder and the parts are Entry Paths, which are the
    // user's own names for their own files (spec: EP-1).
    info!(
        operation = "upload",
        written = written.len(),
        refused = refused.len(),
        bytes,
        freeze = query.freeze,
        "files were added to a folder",
    );
    Ok(Json(UploadDto { written, refused }))
}
