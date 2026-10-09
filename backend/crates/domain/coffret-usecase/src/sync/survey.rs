use coffret_model::EntryPath;

use crate::device_state::LocalObservation;
use crate::sync::candidate::Candidate;
use crate::sync::departed::Departed;
use crate::sync::surfaced::Surfaced;
use crate::unavailable_root::UnavailableRoot;

/// What one scan of the device's mapped folders concluded.
///
/// Everything the rest of the run needs and nothing it does not: the files to
/// encode, the observations to write down for files that turned out unchanged,
/// and the findings to report. No plaintext travels in it — a candidate names a
/// file and the spool step is what opens it — so a survey of a folder of
/// several gigabytes weighs what its Entry Paths weigh.
#[derive(Debug, Default)]
pub(super) struct Survey {
    /// The files to encode, in Entry Path order.
    pub(super) candidates: Vec<Candidate>,
    /// What to write down about files this run found unchanged after reading
    /// them: they were touched, so the length and modification time this device
    /// last saw are stale even though the content is not (spec: EP-10).
    pub(super) refreshed: Vec<LocalObservation>,
    /// How many files were found unchanged, whether or not they had to be read
    /// to establish it.
    pub(super) unchanged: usize,
    /// What the scan surfaces and does not act on (spec: PK-14).
    pub(super) surfaced: Vec<Surfaced>,
    /// The files this device materialized whose Entry has left the Library and
    /// which still hold what this device last made them match, in Entry Path
    /// order: each goes to the trash (spec: EP-15).
    ///
    /// Only files the walk found under a root it could read are here, so every
    /// one of them stands under a root that was there to be read from
    /// (spec: EP-12); whether that root is the one its mapping was recorded
    /// against is asked by the move itself (spec: EP-13).
    pub(super) departed: Vec<Departed>,
    /// The paths whose Entry has left the Library and whose file has left this
    /// device's disk too, whose rows are forgotten rather than reported: nothing
    /// is left anywhere for a finding to be about (spec: EP-15).
    pub(super) forgotten: Vec<EntryPath>,
    /// The mappings whose roots the device cannot vouch for, in mapping order.
    ///
    /// Nothing under one was walked and no deletion was inferred under it, so
    /// this is what keeps a run that scanned less than the mappings cover from
    /// looking like a run that found nothing to do (spec: EP-12, PK-14).
    pub(super) unavailable: Vec<UnavailableRoot>,
}
