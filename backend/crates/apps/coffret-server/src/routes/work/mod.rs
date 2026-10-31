//! The work answer: what the server is doing on its own, in the shapes a
//! browser is told it in, one shape to a file.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::folder::Folder;
use crate::pending_rows::TurnTaker;
use crate::state::ServerState;
use crate::stopping::Stop;

mod add_dto;
use add_dto::AddDto;

mod catalog_dto;
use catalog_dto::CatalogDto;

mod declined_dto;
use declined_dto::DeclinedDto;

mod delete_dto;
use delete_dto::DeleteDto;

mod displaced_fill_dto;
use displaced_fill_dto::DisplacedFillDto;

mod displaced_freeze_dto;
use displaced_freeze_dto::DisplacedFreezeDto;

mod fill_dto;
use fill_dto::FillDto;

mod finding_dto;
use finding_dto::FindingDto;

mod found_dto;
use found_dto::FoundDto;

mod freeze_dto;
use freeze_dto::FreezeDto;

mod kept_parcels_dto;
use kept_parcels_dto::KeptParcelsDto;

mod logged_dto;
use logged_dto::LoggedDto;

mod move_dto;
pub(crate) use move_dto::MoveDto;

mod pair_dto;
pub(crate) use pair_dto::PairDto;

mod reconnect_dto;
use reconnect_dto::ReconnectDto;

mod step_dto;
use step_dto::StepDto;

mod stopped_add_dto;
use stopped_add_dto::StoppedAddDto;

mod sync_dto;
use sync_dto::SyncDto;

mod work_dto;
pub use work_dto::WorkDto;

/// The word a displaced run's status travels under: it is only ever displaced
/// from there.
const STOPPED: &str = "stopped";

/// Folders as a listing spells them: the Library root is the empty string.
fn named_folders(folders: &[Folder]) -> Vec<String> {
    folders
        .iter()
        .map(|folder| folder.as_str().to_owned())
        .collect()
}

/// Whether a run of `flow` that is `running` is waiting for its turn at the
/// pending rows, with `turn` holding them.
///
/// From the moment the run is on record until it holds the turn — which is
/// every running run but the holder's own. Within one flow there is only ever
/// one run on record running, so the holder's flow is the holder's run.
fn waits(running: bool, flow: TurnTaker, turn: Option<TurnTaker>) -> bool {
    running && turn != Some(flow)
}

/// What a run's line offers about stopping it (spec: LA-34): `offered` where
/// a Stop is drawn for it, `asked` where one was pressed and the run has not
/// answered, `committing` where it has begun committing and so takes none —
/// and `null` where the run is not running, or is not one a person can stop.
fn stop_word(running: bool, stoppable: bool, stop: Stop) -> Option<&'static str> {
    (running && stoppable).then_some(match stop {
        Stop::Open => "offered",
        Stop::Asked => "asked",
        Stop::Committing => "committing",
    })
}

/// `GET /api/work`
///
/// Polled while something is happening and not otherwise: an explorer with
/// nothing in flight asks for nothing. An open reader counts as something
/// happening, which is what carries the lock's news to the one screen holding
/// plaintext.
///
/// It needs no key and takes none, so it answers a locked server as readily as
/// an open one.
pub async fn work(State(state): State<Arc<ServerState>>) -> Json<WorkDto> {
    Json(WorkDto::of(&state).await)
}

// Every state this answer can be in, written to the file the explorer's own
// cases read back through its types.
#[cfg(test)]
mod contract;
