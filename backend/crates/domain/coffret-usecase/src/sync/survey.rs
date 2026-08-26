use crate::device_state::LocalObservation;
use crate::sync::candidate::Candidate;
use crate::sync::deferred::Deferred;
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
    pub(super) deferred: Vec<Deferred>,
    /// The mappings whose roots the device cannot vouch for, in mapping order.
    ///
    /// Nothing under one was walked and no deletion was inferred under it, so
    /// this is what keeps a run that scanned less than the mappings cover from
    /// looking like a run that found nothing to do (spec: EP-12, PK-14).
    pub(super) unavailable: Vec<UnavailableRoot>,
}
