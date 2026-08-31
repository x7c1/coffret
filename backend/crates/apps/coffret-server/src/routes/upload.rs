use std::sync::Arc;

use axum::extract::multipart::Field;
use axum::extract::{Multipart, Query, State};
use axum::Json;
use coffret_device::{ContainerKind, EntryPath};
use serde::Serialize;
use tracing::info;

use crate::api_error::ApiError;
use crate::entry_query::{shaped, PathQuery};
use crate::reported::Reported;
use crate::state::ServerState;
use crate::sync::arm_sync;

use super::activity::RefusalDto;

/// What became of one drop.
///
/// Per part, because a drop is a handful of files and they are separate
/// questions: one name the Library holds inside a Pack does not stop the file
/// beside it landing. The two lists are the whole answer — what is now in the
/// folder, and what is not and why.
#[derive(Serialize)]
pub struct UploadDto {
    /// The Entry Paths the files were written at, in the order they arrived.
    ///
    /// Where they will stand in the Library once the sync has carried them in.
    /// They are already in the folder, and the listing shows them from the next
    /// request onwards.
    written: Vec<String>,
    /// The parts nothing was written for, each with the refusal it met.
    refused: Vec<RefusedDto>,
}

/// One part that was not written, named the way the caller named it.
///
/// By the name off the part and not by an Entry Path, because the commonest
/// refusal here is that the name is not one: a part carrying `../etc/passwd` has
/// no Entry Path to be reported under, and answering with one this server had
/// repaired would be answering about a file nobody sent.
#[derive(Serialize)]
struct RefusedDto {
    name: String,
    #[serde(flatten)]
    refusal: RefusalDto,
}

/// `POST /api/upload?path=<folder>`, multipart
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
/// (spec: EP-9) and arms a sync. That is the whole of it: adding a file to a
/// Library has always meant putting it in a mapped folder and letting a sync
/// carry it in, and this is that gesture performed for somebody who is in a
/// browser rather than a file manager. Nothing here encrypts, uploads or commits
/// anything, and no part of the Library changes until the sync commits.
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
/// sync will ever carry in. An Entry in a Container of its own (spec: PK-15) is
/// not refused: a changed mapped file is eligible for `update`, and replacing
/// the one Container holding it is ordinary work (spec: PK-11, PK-12).
///
/// A part that is refused still has its bytes read off the wire and dropped. The
/// alternative is answering in the middle of a request the browser is still
/// sending, which no browser reads.
pub async fn upload(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<PathQuery>,
    mut parts: Multipart,
) -> Result<Json<UploadDto>, ApiError> {
    let folder = query.folder()?;
    // Asked once, of the folder, rather than once per part: the mappings partition
    // the Library by top-level component (spec: EP-9), and every part of a drop
    // onto a folder carries that folder's component — so a folder a mapping
    // reaches leaves no part of the drop unreachable. Only at the Library root do
    // the parts carry components of their own, and there what is asked after is a
    // root mapping, which stands for every component no other mapping claims.
    if !state.library.list(folder.as_ref()).await?.mapped {
        return Err(ApiError::no_folder_here());
    }

    let mut written = Vec::new();
    let mut refused = Vec::new();
    let mut bytes = 0u64;
    while let Some(part) = parts.next_field().await.map_err(ApiError::bad_request)? {
        // A part with no filename is not a file. Nothing this route serves sends
        // one, and inventing a name for it would be inventing an Entry Path.
        let Some(name) = part.file_name().map(str::to_owned) else {
            continue;
        };
        match receive(&state, folder.as_ref(), &name, part).await {
            Ok(landed) => {
                bytes += landed.bytes;
                written.push(landed.path.as_str().to_owned());
            }
            Err(refusal) => refused.push(RefusedDto {
                name,
                refusal: RefusalDto::of(&Reported::recorded(&refusal, "upload")),
            }),
        }
    }

    // Only where something landed. A drop that was refused whole has left the
    // folder exactly as it was, and a sync over an unchanged folder is a walk of
    // every mapped root to find nothing.
    if !written.is_empty() {
        arm_sync(Arc::clone(&state));
    }

    // Counts and sizes. No name of anything reaches the event: the folder and the
    // parts are Entry Paths, which are the user's own names for their own files
    // (spec: EP-1).
    info!(
        operation = "upload",
        written = written.len(),
        refused = refused.len(),
        bytes,
        "files were added to a folder",
    );
    Ok(Json(UploadDto { written, refused }))
}

/// One part that landed.
struct Landed {
    path: EntryPath,
    bytes: u64,
}

/// Takes one part into the folder, or says why it was not taken.
///
/// The order is the point: the name is shaped, the catalog is asked what stands
/// at the path, and only then is anything opened. Everything that can refuse this
/// file has refused it before the first byte is written, so a refusal never
/// leaves a partial file behind — and a failure part way through does not either,
/// because the bytes are going to a temporary name that is removed when the
/// incoming file is dropped (spec: EP-11).
async fn receive(
    state: &Arc<ServerState>,
    folder: Option<&EntryPath>,
    name: &str,
    mut part: Field<'_>,
) -> Result<Landed, ApiError> {
    let path = under(folder, &shaped(name)?);
    if state.library.container_of(&path).await? == Some(ContainerKind::Pack) {
        return Err(ApiError::pack_resident());
    }

    let mut incoming = state.library.receive_file(&path).await?;
    while let Some(chunk) = part.chunk().await.map_err(ApiError::bad_request)? {
        incoming.write(&chunk).await?;
    }
    let bytes = incoming.written();
    incoming.keep().await?;
    Ok(Landed { path, bytes })
}

/// Where a part named relative to `folder` stands in the Library.
///
/// Both halves are already the Library's spelling — the folder came through the
/// same shaping and the relative path did too — so composing them changes nothing
/// (spec: EP-1).
fn under(folder: Option<&EntryPath>, relative: &EntryPath) -> EntryPath {
    match folder {
        None => relative.clone(),
        Some(folder) => EntryPath::nfc(format!("{}/{}", folder.as_str(), relative.as_str())),
    }
}
