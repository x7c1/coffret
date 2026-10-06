//! The work answer: what the server is doing on its own, in the shapes a
//! browser is told it in, one shape to a file.

use std::sync::Arc;

use axum::extract::State;
use axum::Json;

use crate::folder::Folder;
use crate::state::ServerState;

mod catalog_dto;
use catalog_dto::CatalogDto;

mod declined_dto;
use declined_dto::DeclinedDto;

mod displaced_fill_dto;
use displaced_fill_dto::DisplacedFillDto;

mod displaced_freeze_dto;
use displaced_freeze_dto::DisplacedFreezeDto;

mod fill_dto;
use fill_dto::FillDto;

mod finding_dto;
use finding_dto::FindingDto;

mod freeze_dto;
use freeze_dto::FreezeDto;

mod reconnect_dto;
use reconnect_dto::ReconnectDto;

mod step_dto;
use step_dto::StepDto;

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
    Json(WorkDto::of(&state))
}

// Every state this answer can be in, written to the file the explorer's own
// cases read back through its types.
#[cfg(test)]
mod contract;
