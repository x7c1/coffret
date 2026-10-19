use crate::device_state::{LocalObservation, ReservationId};
use crate::freeze::lapsed::Lapsed;
use crate::freeze::not_frozen::NotFrozen;
use crate::freeze::selected::Selected;
use crate::unavailable_root::UnavailableRoot;

/// What one scan of the folder concluded.
///
/// No plaintext travels in it — a selection names a file and the spool step is
/// what opens it again — so a survey of a folder of several hundred gigabytes
/// weighs what its Entry Paths and hashes weigh.
pub(super) struct Survey {
    /// The files to pack, in Entry Path order (spec: PK-3).
    pub(super) selected: Vec<Selected>,
    /// How many Entries a Pack already holds and the local file still matches.
    pub(super) packed_already: usize,
    /// What the scan surfaces and does not act on (spec: PK-14).
    pub(super) surfaced: Vec<NotFrozen>,
    /// The mappings whose roots the device cannot vouch for, in mapping order.
    ///
    /// Nothing under one was walked, so it contributed no candidate — and the
    /// only harm it could do a freeze is silence, which is what this stops
    /// (spec: EP-12, PK-14).
    pub(super) unavailable: Vec<UnavailableRoot>,
    /// What to write down about files that turned out unchanged after being
    /// read: they were touched, so the length and modification time this device
    /// last saw are stale even though the content is not (spec: EP-10).
    pub(super) refreshed: Vec<LocalObservation>,
    /// The reservations this run decided every path of, in the order they
    /// were made (spec: PK-17).
    pub(super) closing: Vec<Closing>,
}

/// What a run makes of one reservation all of whose paths it considered
/// (spec: PK-17).
///
/// A reservation with a path the run did not consider — outside its folder or
/// its selection, or under a mapped root it could not vouch for (spec: EP-12) —
/// is not among them: what becomes of that path is a later run's to decide, so
/// the reservation stays.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Closing {
    /// The run packs at least one of its paths, and the commit that does is
    /// what releases it.
    Packs(ReservationId),
    /// None of its paths was eligible any more: it is released on its own,
    /// and these say why.
    Lapses(ReservationId, Vec<Lapsed>),
}
