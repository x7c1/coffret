//! The two routes that carry anything into the Library: the upload, which
//! takes the bytes a browser's drop sends, and the import, which takes files
//! the desktop app's window dropped by path (`import`).
//!
//! The upload itself is here with the shapes it is asked and answered with;
//! taking one file of a drop, by either route, is `receive`, and where a
//! refusal about one file and a refusal about the whole request are told apart
//! is `refusal`.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::extract::{Multipart, Query, State};
use axum::http::HeaderMap;
use axum::Json;

use crate::api_error::ApiError;
use crate::entry_query::folder_named;
use crate::folder::Folder;
use crate::reported::Reported;
use crate::state::ServerState;

use super::RefusalDto;

use self::declared_length::declared_length;
use self::drop_reservation::DropReservation;
use self::finish_drop::{finish_drop, FinishedDrop};
use self::outran::outran;
use self::receive::{receive, Adding, Reserving};
use self::refusal::Refusal;
use self::refused_dto::RefusedDto;
use self::release::release_stopped;
use self::stop_receiving::stop_receiving;

mod declared_length;

mod drop_reservation;

mod finish_drop;

// The same gesture made by path, from the desktop app's window: the files are
// read off this device rather than sent, and taken through the same `receive`.
mod import;
pub use import::import;

mod landed;

mod modified;

mod outran;

mod receive;

mod refusal;

mod refused_dto;

mod release;

mod room_for;

mod stop_receiving;

mod under;

mod upload_dto;
pub use upload_dto::UploadDto;

mod upload_query;
pub use upload_query::UploadQuery;

/// What every event of a browser's drop names its operation as.
const OPERATION: &str = "upload";

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
/// A part's field name is the file's own modification time, in whole
/// milliseconds from the Unix epoch, which the file is stamped with; a name
/// that is not such a count — `file`, say — leaves the file with the time it
/// is written at and is not refused (spec: EP-11). `modified` says how it is
/// read.
///
/// # What it does, and what it deliberately is not
///
/// It writes files into the folder this device maps that subtree of the Library
/// into (spec: EP-9) and arms the work that carries them in. That is the whole
/// of it: adding a file to a Library has always meant putting it in a mapped
/// folder and letting a flow carry it in, and this is that gesture performed for
/// somebody who is in a browser rather than a file manager. Nothing here
/// encrypts, uploads or commits anything, and nothing in the Library changes
/// until that flow commits.
///
/// # Which flow, and why the browser says
///
/// A plain drop arms a sync, which is the same gesture as copying the files in
/// and typing `coffret sync`: one Container per file, which is the right shape
/// for the handful of files a drop usually is.
///
/// `?freeze=true` arms a freeze instead (spec: PK-17), and is what the explorer
/// sends for a dropped folder the person chose to add as a Pack. That is a book
/// being brought in — a folder of a few hundred page images arriving in one
/// gesture — and a sync would make it a few hundred Storage Objects, a few
/// hundred uploads, and a few hundred provider calls to open again. The freeze
/// packs them instead, so they go up once, as Packs.
///
/// What it packs is exactly what this request wrote, and not the folder: the
/// freeze is armed with the Entry Paths that landed as its selection
/// ([`Book::dropped`]), so a folder the Library already has keeps the one-file
/// Entries it held before the drop in the Containers they were in. Where the
/// selection names a path the run finds ineligible — one a Pack already holds,
/// say — the run decides that, file by file, as it always does (spec: PK-1).
/// And it is what lets a drop onto the Library root ask for a Pack: the
/// selection, not the folder, bounds the run, so it cannot become the whole
/// Library.
///
/// Which of the two it is comes from the caller and is not worked out here, for
/// the reason [`UploadQuery`] gives: the person was asked.
///
/// # A choice of a Pack outlives the process
///
/// The freeze a drop arms is this process's own work, and a process that stops
/// — a desktop app quit, or a crash, which this server cannot tell apart — takes
/// it along (spec: LA-12). The pages it leaves in the folder would then read to
/// the next sync as loose files, and go up one Container each. So the choice is
/// written down on this device before the first page lands: a reservation of
/// the folder, made before the first part is read and extended by each page
/// before it lands (spec: PK-17). A sync leaves a reserved page alone and says
/// it waits for its Pack. Until the drop has finished receiving, the
/// reservation is a drop in progress: no freeze packs any of its pages or
/// decides it — a run of the whole folder going on beside the drop leaves both
/// for the drop's own run. That run is armed once receiving ends and the
/// reservation is marked received, as a run of its own rather than one taken
/// for the run under way ([`freeze_dropped`]), and the freeze that packs the
/// pages releases the reservation with its commit. A server started afterwards
/// takes up every received reservation it finds, in the order they were made
/// ([`resume_reserved`](crate::freeze::resume_reserved)); one whose drop was
/// cut off by the process stopping stays where it is, and the next drop onto
/// its folder that asks for a Pack takes it over, so the pages the first drop
/// received and the ones the second brings are packed as one book
/// (spec: LA-12). A drop refused as a whole, or one every part of which was
/// refused, arms nothing, and a reservation it made is released with it: what
/// it left in the folder waits there as anything copied into a mapped folder
/// does. A reservation it took over is not let go that way: one refused as a
/// whole stays in progress, for the next drop of its folder, and one every part
/// of which was refused still finishes the book the cut-off drop received.
///
/// # Refused before anything lands
///
/// The drop's own refusals are about its folder and about its parts one by
/// one, and every one of them is decided before a byte reaches disk. A folder no
/// mapping of this device reaches takes the whole drop with it: there is nowhere
/// to put any of it (spec: EP-9), and the listing has already said so over the
/// rows. A part whose relative path is not an Entry Path is refused by name
/// (spec: EP-2). A part carrying a name coffret keeps for itself — its scratch
/// prefix, or the device's own management area — is refused by name too, because
/// a file written under either would sit in the folder and never reach the
/// Library (spec: EP-11, EP-14). A part whose way down from its mapped root
/// passes through something that is not a real folder of that root — a symbolic
/// link, or a file where a folder must be — is refused where the descent meets
/// it, because a file written through it would land somewhere the mappings never
/// named (spec: EP-4, EP-11). And a part standing where the Library holds an
/// Entry inside a Pack is refused by name too, because coffret cannot yet
/// replace one (spec: PK-10, PK-12) — writing it would leave a file in the
/// folder that no flow will ever carry in. An Entry in a Container of its own
/// (spec: PK-15) is not refused: a changed mapped file is eligible for `update`,
/// and replacing the one Container holding it is ordinary work
/// (spec: PK-11, PK-12).
///
/// A part refused for itself — by its name, or where the descent below a sound
/// root meets a fence — still has its bytes read off the wire and dropped. The
/// alternative is answering in the middle of a request the browser is still
/// sending, which no browser reads, and what reading the part out buys is the
/// rest of the drop, which is still going. A refused root buys nothing by
/// it: in a drop onto a folder every part that follows meets that same root,
/// so there is no rest of the drop for reading this part out to protect, and
/// it stops the request where it stands with the budgets below.
///
/// # Refused because of what it would cost
///
/// The refusals above are about the drop itself. What stops the request is
/// about this server and this device: the three budgets this server takes a
/// drop within (spec: LA-9) — how much one request may carry, how much one
/// part of it may, and how many parts there may be — and a mapped root that
/// will not vouch for itself. Passing a budget is not a part being refused; it
/// stops the request where it stands (spec: LA-10), because a request that has
/// already passed a budget is one whose remaining bytes there is no reason to
/// read. A part whose mapped folder is not the folder its mapping was recorded
/// against stops it for that same reason rather than a different one: nothing
/// is placed into a root that will not vouch for itself (spec: EP-13), and a
/// drop onto a folder goes through the one root that folder's mapping names, so
/// the refusal is no truer of the next part than of this one. Only a drop onto
/// the Library root can carry parts under mappings of their own (spec: EP-9),
/// and such a drop stops at the first refused root among them the way a
/// declined placement fails a single writer's request (spec: EP-11), leaving
/// what landed before it where a passed budget leaves it: in the folder, with
/// nothing armed.
///
/// So these are the places the route does what it will not do for a part refused
/// for itself: answer in the middle of a request the browser is still sending,
/// and pay for it — what reaches the person may be a transfer that failed rather
/// than the sentence it was answered with. What makes that worth paying here and
/// not for a part refused for itself is the size of the other side. Reading a
/// refused part to the end costs what that one part costs and keeps the rest of
/// the drop going; reading out a request that has already passed a budget is
/// doing the whole of the thing the budget is there to refuse, and reading out a
/// drop into a refused root buys nothing at all, so the one part's cost is in
/// fact the whole request's. So the sentence is written for whoever does read
/// it, and every one of these refusals is put in the log as well — that is the
/// half that always arrives. Where the answer is read, it names in `written`
/// the files that had landed before the stop, as an answer that took the drop
/// would have.
///
/// Beside them is a question rather than a budget: whether the volume this
/// device's folder is on still has room for what is coming. It is asked of each
/// part before that part is taken, so a drop that would fill the disk is refused
/// while refusing is still cheap (spec: LA-11).
///
/// # One drop at a time, and why nothing here enforces it
///
/// The explorer drops one book and waits for the answer, and nothing on this
/// route serialises anything. That is deliberate rather than missing. Two drops
/// arriving at once write into different files and refuse each other nothing —
/// each part goes to a scratch name of its own and is renamed onto its
/// destination (spec: EP-11) — and what they arm behind them is already ordered
/// where ordering matters: a second book armed while one is being packed waits
/// its turn behind it, which is [`freeze_dropped`]'s doing — a freeze commits
/// one batch (spec: PK-7), so a book put aside half way is one that was never
/// brought in at all — and a second sync collapses into the one walk of the
/// mappings that would have found both drops anyway. A queue in front of this
/// route would add nothing to either and would make a person dropping a
/// photograph wait behind somebody's book.
///
/// # An add, which a person can stop
///
/// The drop is one **add** (spec: LA-34), named by `?add=` where the page gave
/// it a name — the page does, so that its Stop can name it while the files
/// are still being sent — and by one drawn here where it did not. A stop
/// asked while the drop is receiving is met before the next part, or between
/// two pieces of the one arriving, and the drop is answered `cancelled` with
/// nothing in `written`: what it wrote is undone on this device
/// ([`stop_receiving`]), and what that came to is logged with the server's
/// finished work (spec: LA-36). A stop asked once the drop has armed its run
/// is that run's to answer.
pub async fn upload(
    State(state): State<Arc<ServerState>>,
    Query(query): Query<UploadQuery>,
    headers: HeaderMap,
    mut parts: Multipart,
) -> Result<Json<UploadDto>, ApiError> {
    // Before a single part is read, beside the refusals below and for the
    // same reason: a drop that lands its files and then finds nothing can carry
    // them in is the one state a person must not be put in silently
    // (spec: DK-2).
    let library = state.unlocked()?;
    let folder = folder_named(query.path.as_deref())?;
    // Asked once, of the folder, rather than once per part: the mappings partition
    // the Library by top-level component (spec: EP-9), and every part of a drop
    // onto a folder carries that folder's component — so a folder a mapping
    // reaches leaves none of the drop unreachable. Only at the Library root do
    // the parts carry components of their own, and there what is asked after is a
    // root mapping, which represents every component no other mapping does.
    if !library.list(folder.as_ref()).await?.mapped {
        return Err(ApiError::no_folder_here());
    }

    // What the request says it is bringing, where it says anything. Every browser
    // sending a `FormData` does; a caller that streams without saying is not
    // refused for it, and the fence below asks for the room one part could take
    // instead of the room the rest of the request will.
    let declared = declared_length(&headers);

    // Before the first part, so that no page of a drop that asked for a Pack
    // stands in the folder without its reservation (spec: PK-17). A
    // reservation that cannot be written refuses the drop before anything
    // lands, for the reason the `unlocked` above does: a page that lands
    // unreserved is one a sync carries in on its own. Held live for as long as
    // the drop is receiving, so that no freeze in this process decides it while
    // pages are still to come. Where a drop onto this folder was cut off part
    // way, this one takes its reservation over and goes on adding to it
    // (`DropReservation::hold`). `reserved` is the paths it names as Entry
    // Paths, for the freeze's selection: what the reservation names is what
    // the drop asked to pack, and nothing else in the folder is.
    let add = state.adds.begin(
        query.add.as_deref(),
        Folder::named(folder.clone()),
        query.freeze,
    )?;
    let (reservation, mut reserved) = match query.freeze {
        true => match DropReservation::hold(&library, folder.as_ref(), OPERATION).await {
            Ok((held, received)) => (Some(held), received),
            Err(refused) => {
                state.adds.end(&add);
                return Err(refused);
            }
        },
        false => (None, BTreeSet::new()),
    };
    let adding = Adding {
        adds: &state.adds,
        id: &add,
    };

    let mut written = Vec::new();
    let mut refused = Vec::new();
    let mut bytes = 0u64;
    let mut seen = 0usize;
    // Every way the loop below stops the drop comes out of this one block, so
    // that every one of them — a budget passed, no room, a refused root, a
    // stream that broke — is answered with what had landed by then. What it
    // answers with is whether a person stopped the add.
    let taken: Result<bool, ApiError> = async {
        loop {
            if adding.stopping() {
                return Ok(true);
            }
            let Some(part) = parts.next_field().await.map_err(ApiError::multipart)? else {
                return Ok(false);
            };
            // Counted before it is looked at, and every part counts — one with no
            // filename included. What this budget is about is the request, and a
            // request made of a million parts this route would skip is still a
            // million parts to read.
            seen += 1;
            if seen > state.allowance.parts {
                return Err(outran(
                    OPERATION,
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
            let coming =
                declared.map_or(state.allowance.part_bytes, |all| all.saturating_sub(bytes));
            let reserving = reservation.as_ref().map(|held| Reserving {
                id: held.id(),
                paths: &mut reserved,
            });
            match receive(
                &library,
                &state.allowance,
                coming,
                folder.as_ref(),
                &name,
                reserving,
                &adding,
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
                    refusal: RefusalDto::of(&Reported::recorded(&refusal, OPERATION)),
                }),
                // Nothing is armed for what landed before it: this request was
                // refused, and arming its work would be answering a refusal with
                // the flow it asked for. Those files are in the folder and whole,
                // and they wait there as anything else copied into a mapped folder
                // waits — nothing on this server arms a sync on its own, so what
                // carries them in is a later drop that lands something, or somebody
                // asking for one.
                Err(Refusal::Request(refusal)) => return Err(refusal),
                // The stream broke while that part was in flight. Said the way the
                // `next_field` above says a stream that broke between parts, which
                // is the one way this route says it — rather than by asking the
                // broken stream for another part, which is not certain to fail the
                // same way twice. What it is answered with goes into a connection
                // that is most likely gone; what was written of the part went to a
                // scratch name that was removed with it (spec: EP-11), and nothing is
                // armed for what landed before it, for the reason the arm above gives.
                Err(Refusal::Interrupted(cause)) => return Err(ApiError::multipart(cause)),
                Err(Refusal::Cancelled) => return Ok(true),
            }
        }
    }
    .await;
    let cancelled = match taken {
        Ok(cancelled) => cancelled,
        // A request whose page went away mid-stop — it was reloaded, say — is
        // still the stop it asked for, and is answered as one in the log.
        Err(_) if adding.stopping() => true,
        Err(stopped) => {
            // Nothing is armed for a drop that was stopped, so nothing is to wait
            // for a Pack either — save the files of a drop cut off earlier that
            // this one took over, which still do (spec: PK-17). What landed
            // waits in the folder as anything copied there does, and the add
            // is over.
            release_stopped(&library, reservation, OPERATION).await;
            state.adds.end(&add);
            return Err(stopped.having_written(written));
        }
    };
    if cancelled {
        stop_receiving(
            &state,
            &library,
            add,
            reservation,
            !query.freeze,
            !written.is_empty(),
            OPERATION,
        )
        .await;
        return Ok(Json(UploadDto {
            written: Vec::new(),
            refused,
            cancelled: true,
        }));
    }
    finish_drop(
        &state,
        &library,
        FinishedDrop {
            operation: OPERATION,
            add,
            reservation,
            reserved,
            written: written.len(),
            refused: refused.len(),
            bytes,
            freeze: query.freeze,
        },
    )
    .await;
    Ok(Json(UploadDto {
        written,
        refused,
        cancelled: false,
    }))
}
