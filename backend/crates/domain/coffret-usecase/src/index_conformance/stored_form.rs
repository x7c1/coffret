use coffret_model::ObjectRef;

use crate::device_state::PendingRow;

/// Writes into an implementation's stored form past the port, for the cases
/// about what the port cannot express.
///
/// Only an implementation whose stored form can hold such a thing hands one
/// over: one whose stored form is the domain value itself has nothing to plant.
pub trait StoredForm: Send + Sync {
    /// Stores `row`, which is [`Spooling`](crate::device_state::SpoolState::Spooling),
    /// as naming `object` besides — a pair no writer of the catalog produces.
    fn plant_spooling_row_with_object(&self, row: &PendingRow, object: &ObjectRef);
}
