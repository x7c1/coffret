//! The drop the desktop app's window makes by path.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::Json;

use crate::api_error::ApiError;
use crate::entry_query::folder_named;
use crate::reported::Reported;
use crate::routes::RefusalDto;
use crate::state::ServerState;

use super::finish_drop::{finish_drop, FinishedDrop};
use super::outran::outran;
use super::receive::{receive, Reserving};
use super::refusal::Refusal;
use super::refused_dto::RefusedDto;
use super::release::release;
use super::UploadDto;

use self::held_file::HeldFile;

mod held_file;

mod import_request;
pub use import_request::ImportRequest;

/// What every event of an import names its operation as.
const OPERATION: &str = "import";

/// `POST /api/import` with `{ "path": …, "freeze": …, "handle": …, "files":
/// [{ "index": …, "name": … }] }`
///
/// Adds files a drop held by path to one folder of the Library: the same
/// gesture as [`upload`](super::upload), made from the desktop app's window,
/// where what was let go of is a set of paths on this device rather than bytes
/// a browser read. The app walked them and holds what it took under a one-time
/// drop handle ([`Drops`](coffret_device::Drops)); the page was told the
/// handle and, per file, its path relative to the folder it was dropped from,
/// and names here which of them go in, by their index in that set, and under
/// what name relative to the folder.
///
/// Nothing in the body is a path on this device, and nothing could be: every
/// field is one of the four above, a body carrying any other is refused, and a
/// file is addressed by its index alone (spec: LA-27). The handle is used up by
/// the request that names it, whatever that request comes to, and one that is
/// unknown, used or expired is refused before anything is read, as is an index
/// outside the held set (spec: LA-26, LA-27). A request refused before the
/// handle is read — the Library locked, or a body this route does not read —
/// leaves the drop held.
///
/// From there it is the upload, and through the same code: the folder is asked
/// once whether a mapping reaches it, a Pack is reserved before the first file
/// lands where one was asked for (spec: PK-17), each file is taken by
/// [`receive`] — the room this device has for what is still coming, the name
/// and fence refusals, the write to a scratch and the rename — and the flow
/// that carries what landed in is armed the same way. It answers in the
/// upload's shape, so the explorer reads both alike.
///
/// What bounds it is not the upload's three budgets, which are about what one
/// socket carries (spec: LA-9): the bytes here are read off this device's own
/// disk. It is the room this device has, asked of each file before it is
/// taken; how many files one request names, at most as many as an upload may
/// carry parts; and the held set, which no request can add to (spec: LA-28).
/// Each file is read again through the same confined reader a freeze reads a
/// source through, so one replaced by a link since it was dropped is refused
/// rather than followed (spec: LA-29), and one that grew since is refused part
/// way through, leaving nothing behind (spec: EP-11).
pub async fn import(
    State(state): State<Arc<ServerState>>,
    body: Result<Json<ImportRequest>, JsonRejection>,
) -> Result<Json<UploadDto>, ApiError> {
    // Before the handle is used, so that a drop onto a locked server can be
    // made again once it is unlocked (spec: DK-2).
    let library = state.unlocked()?;
    let Json(request) = body.map_err(|rejection| ApiError::unreadable_json(&rejection))?;
    // Taken out before anything else about the request is read: a handle is
    // used once, whatever the request that named it comes to (spec: LA-26).
    let held = state
        .drops
        .take(&request.handle)
        .map_err(ApiError::drop_not_held)?;
    if request.files.len() > state.allowance.parts {
        return Err(outran(
            OPERATION,
            "one drop is one gesture, and this names more files than one gesture takes — \
             the same files in two drops are taken",
        ));
    }
    let mut sources = Vec::with_capacity(request.files.len());
    for asked in &request.files {
        let file = held
            .file(asked.index)
            .ok_or_else(ApiError::not_in_the_drop)?;
        sources.push((file, asked.name.as_str()));
    }
    let folder = folder_named(request.path.as_deref())?;
    // Asked once, of the folder, for the reason the upload gives.
    if !library.list(folder.as_ref()).await?.mapped {
        return Err(ApiError::no_folder_here());
    }
    let reservation = match request.freeze {
        true => Some(library.reserve(folder.as_ref()).await?),
        false => None,
    };

    let mut written = Vec::new();
    let mut reserved = BTreeSet::new();
    let mut refused = Vec::new();
    let mut bytes = 0u64;
    // What is still to come, for the room this device is asked to have: the
    // lengths the walk stated, less each one as it is taken.
    let mut coming: u64 = sources.iter().map(|(file, _)| file.size()).sum();
    let taken: Result<(), ApiError> = async {
        for (file, name) in &sources {
            let refuse = |refusal: ApiError, refused: &mut Vec<RefusedDto>| {
                refused.push(RefusedDto {
                    name: (*name).to_owned(),
                    refusal: RefusalDto::of(&Reported::recorded(&refusal, OPERATION)),
                });
            };
            let reserving = reservation.as_ref().map(|live| Reserving {
                id: live.id(),
                paths: &mut reserved,
            });
            let outcome = match HeldFile::open(file).await {
                Ok(source) => {
                    receive(
                        &library,
                        &state.allowance,
                        coming,
                        folder.as_ref(),
                        name,
                        reserving,
                        source,
                    )
                    .await
                }
                Err(refusal) => Err(Refusal::Part(refusal)),
            };
            coming = coming.saturating_sub(file.size());
            match outcome {
                Ok(landed) => {
                    bytes += landed.bytes;
                    written.push(landed.path.as_str().to_owned());
                }
                Err(Refusal::Part(refusal)) => refuse(refusal, &mut refused),
                // Nothing is armed for what landed before it, for the reason
                // the upload gives.
                Err(Refusal::Request(refusal)) => return Err(refusal),
                // No body is being read, so no stream can break under a file;
                // were one to, it is the request that stops.
                Err(Refusal::Interrupted(cause)) => return Err(ApiError::multipart(cause)),
            }
        }
        Ok(())
    }
    .await;
    if let Err(stopped) = taken {
        release(&library, reservation, OPERATION).await;
        return Err(stopped.having_written(written));
    }
    finish_drop(
        &state,
        &library,
        FinishedDrop {
            operation: OPERATION,
            reservation,
            reserved,
            written: written.len(),
            refused: refused.len(),
            bytes,
            freeze: request.freeze,
        },
    )
    .await;
    Ok(Json(UploadDto { written, refused }))
}
