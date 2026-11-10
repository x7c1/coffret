use serde::Serialize;

use crate::moves::PendingMove;
use crate::pending_rows::TurnTaker;

use super::PairDto;

/// One move accepted and not yet ended, as the work answer lists it
/// (spec: LA-31).
#[derive(Serialize)]
pub(super) struct PendingMoveDto {
    /// Which move this is: the number the `POST` that asked for it answered
    /// with, and the one its log entry carries once it has ended.
    run: u64,
    /// The pairs it was asked to move.
    pairs: Vec<PairDto>,
    /// Whether it is waiting: behind another move in the server's queue, or,
    /// taken up, for its turn at this device's pending rows while another
    /// flow holds them. False once it holds the turn, from its catch-up to its
    /// relocations.
    waits: bool,
}

impl PendingMoveDto {
    pub(super) fn of(pending: &PendingMove, turn: Option<TurnTaker>) -> Self {
        Self {
            run: pending.run,
            pairs: pending.pairs.iter().map(PairDto::of).collect(),
            waits: !pending.taken || turn != Some(TurnTaker::Move),
        }
    }
}
