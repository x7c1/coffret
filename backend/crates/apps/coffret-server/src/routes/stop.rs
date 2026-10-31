use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::adds::AddId;
use crate::api_error::ApiError;
use crate::state::ServerState;

use super::work::WorkDto;

/// What a Stop names: an add, the run on one of the three lines that draw
/// one, by the run number its line was drawn from, or a book or a folder
/// still waiting in its flow's queue, by its folder (spec: LA-35).
///
/// Externally tagged, so a body is `{"add": "<name>"}`, `{"sync": 3}`,
/// `{"freeze": 2}`, `{"fill": 5}`, `{"queued_freeze": "<folder>"}` or
/// `{"queued_fill": "<folder>"}`, a folder spelled as a listing spells it. A
/// deletion and a move are not among them: neither can be stopped
/// (spec: LA-34), and a body naming one is refused as unreadable.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum StopRequest {
    /// The add the page named as it made it: a drop, or a **+**.
    Add(String),
    /// The sync run on record, which a person stops through the adds it
    /// carries — every one of them.
    Sync(u64),
    /// The freeze run on record, and every add whose files it packs.
    Freeze(u64),
    /// The fill run on record.
    Fill(u64),
    /// The book of this folder waiting in the freeze's queue, behind the run
    /// on record.
    QueuedFreeze(String),
    /// The folder waiting in the fill's queue, behind the run on record.
    QueuedFill(String),
}

/// `POST /api/stop`
///
/// Asks for an add, a freeze or a fill to be stopped, and answers at once
/// with the work answer: a stop is a request, and the run answers it by
/// ending (spec: LA-35). Until then the run's line says it was asked
/// (`stop: "asked"`), and once it has ended the log says what it came to: an
/// add undone on this device (spec: LA-36), or a freeze or a fill
/// `cancelled`. A book or a folder still waiting in its queue leaves it, and
/// is logged `cancelled` — at once where nothing of it is to be undone, and
/// for a book holding adds once the run that takes it up has undone them. A
/// run that has begun committing takes no stop — its line says
/// `committing` and draws no Stop — and one asked anyway is answered by what
/// it came to.
///
/// A stop naming nothing under way, or a run that is not the one on record
/// any more, changes nothing: the page that pressed it was looking at an
/// answer older than this one, which tells it how things stand now. Keyed like
/// every route (spec: LA-2), and refused while the Library is locked as every
/// keyed route is (spec: DK-2): undoing an add reads the catalog.
///
/// One stop is refused outright, `409 conflict`, marking nothing: of an add —
/// or of a run carrying one — that wrote over a file this device could not
/// keep aside, since undoing it could not put that file back. The refusal
/// names the file, and the add goes on to its run (spec: LA-36).
pub async fn stop(
    State(state): State<Arc<ServerState>>,
    body: Result<Json<StopRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<WorkDto>), ApiError> {
    state.unlocked()?;
    let Json(request) = body.map_err(ApiError::unreadable_json)?;
    match request {
        StopRequest::Add(name) => {
            if let Some(id) = AddId::named(&name) {
                stop_add(&state, &id)?;
            }
        }
        StopRequest::Sync(run) => {
            // A sync is stopped only through the adds it carries, so one
            // carrying none is not stopped at all.
            if let Some(adds) = state.syncs.carried_by(run).filter(|adds| !adds.is_empty()) {
                state.adds.ask_stop_all(&adds)?;
                state.syncs.ask_stop();
            }
        }
        StopRequest::Freeze(run) => {
            if let Some(adds) = state.freezes.carried_by(run) {
                state.adds.ask_stop_all(&adds)?;
                state.freezes.ask_stop_whole();
            }
        }
        StopRequest::Fill(run) => {
            state.fills.ask_stop(run);
        }
        StopRequest::QueuedFreeze(folder) => stop_queued_book(&state, &folder)?,
        StopRequest::QueuedFill(folder) => {
            state.fills.cancel_waiting(&folder);
        }
    }
    Ok((StatusCode::ACCEPTED, Json(WorkDto::of(&state).await)))
}

/// Marks one add stopped, and asks the run that is to answer it.
///
/// An add still receiving is answered by its own request, between two of its
/// files. One armed for a freeze is answered by the run packing it, where that
/// run is the one on record, and otherwise by its book's run as that starts.
/// One for a sync — receiving or armed — asks the sync on record as well,
/// whichever run carries the add: a sync walks every mapped folder, so the run
/// under way may be about to read what is being taken away.
///
/// Refused, asking nothing, for an add that wrote over a file this device
/// could not keep aside (spec: LA-36).
fn stop_add(state: &ServerState, id: &AddId) -> Result<(), ApiError> {
    let Some(add) = state.adds.ask_stop(id)? else {
        return Ok(());
    };
    match add.freeze {
        true if !add.receiving && state.freezes.carries(id) => {
            state.freezes.ask_stop();
        }
        true => {}
        false => {
            state.syncs.ask_stop();
        }
    }
    Ok(())
}

/// Stops the book of `folder` waiting in the freeze's queue (spec: LA-34).
///
/// One holding no add is a folder somebody asked to have packed, of which
/// nothing has been walked: it leaves the queue at once, logged `cancelled`.
/// One holding adds is stopped through them, as a stop of the run carrying
/// them is (spec: LA-36): each is marked stopped, and the book stays queued,
/// with nothing else of it to pack, for the run that takes it up to undo them
/// under its turn before it reads anything. A book the worker takes up between
/// the adds being read and the book being marked is stopped as its run, as a
/// Stop on that run's line is. Either way a request of the book's own is
/// stopped with it: the folder is not armed again, and a `cancelled` freeze of
/// it is logged beside the adds (spec: LA-12). Refused, asking nothing, where
/// one of those adds wrote over a file this device could not keep aside.
fn stop_queued_book(state: &ServerState, folder: &str) -> Result<(), ApiError> {
    let Some(adds) = state.freezes.waiting_adds(folder) else {
        return Ok(());
    };
    if adds.is_empty() {
        state.freezes.cancel_waiting(folder);
        return Ok(());
    }
    state.adds.ask_stop_all(&adds)?;
    // Where the worker took the book up meanwhile, the run on record carries
    // the adds now and is the one to answer them — asked as a Stop on its own
    // line is, since this is still a stop of that book.
    state.freezes.stop_queued(folder, &adds);
    Ok(())
}
