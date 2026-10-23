use std::collections::BTreeSet;

use coffret_model::EntryPath;

use super::ReservationId;

/// A drop's choice to add its files as a Pack, recorded on this device so that
/// it outlives the process that took the drop (spec: PK-17, LA-12).
///
/// Device state like a mapping or a pending row: never uploaded, never part of
/// an Index Snapshot (spec: CK-7). It is written before the first file of the
/// drop lands and extended as each one does, so a file the drop wrote is never
/// standing in the mapped folder without the reservation that says it waits for
/// a Pack — which is what keeps a sync from carrying it in one Container per
/// file in between.
///
/// It is *received* once the drop has stopped receiving with every file it was
/// to bring: only then does it name the whole book, and only then may a freeze
/// decide it. One that is not — a drop still landing, or one cut off by the
/// process that took it stopping — is a drop in progress: no freeze packs any
/// of its paths, and a drop onto the same folder takes it over and goes on
/// adding to it, so the book still comes to one Pack (spec: PK-17, LA-12).
///
/// It goes when a freeze that considered all of its paths has decided each of
/// them: with the commit that packs any of them, or on its own, with a finding,
/// where none of them was eligible any more.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reservation {
    /// Which reservation this is.
    pub id: ReservationId,
    /// The folder the drop was made onto, and `None` for the Library root.
    pub folder: Option<EntryPath>,
    /// The Entry Paths the drop has written, or was about to write when it
    /// stopped, and asked to pack.
    pub paths: BTreeSet<EntryPath>,
    /// Whether the drop finished receiving, so that `paths` is the whole of
    /// what it brought. `false` on one made before the mark was kept, which
    /// is read as cut off.
    pub received: bool,
}

impl Reservation {
    /// Whether this is a drop in progress: one a drop is still landing pages
    /// into, or one cut off before it finished receiving (spec: PK-17).
    pub fn in_progress(&self) -> bool {
        !self.received
    }
}
