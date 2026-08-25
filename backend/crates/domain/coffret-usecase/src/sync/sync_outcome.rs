use coffret_model::ContainerId;

use crate::commit::CommitOutcome;
use crate::sync::deferred::Deferred;
use crate::sync::reconciled::Reconciled;
use crate::unavailable_root::UnavailableRoot;

/// What one sync run found and what it did about it.
///
/// Two halves, and the second is the one that matters most. [`commit`] says
/// what became of the Library, and a run that found nothing to upload carries
/// `None` there rather than an empty commit — a Journal record for a batch that
/// changes nothing is a generation spent on nothing (spec: CP-1). [`deferred`]
/// says what the run left alone, and it is not an afterthought: a scan
/// selecting update candidates has to surface every file that needs one, so a
/// caller reads this list rather than assuming that a successful sync means
/// every local file is backed up (spec: PK-14). [`unavailable`] is the other
/// half of that obligation and is read the same way.
///
/// [`commit`]: Self::commit
/// [`deferred`]: Self::deferred
/// [`unavailable`]: Self::unavailable
#[derive(Debug)]
pub struct SyncOutcome {
    /// The Containers this run uploaded and committed, one per file — the
    /// imports and the replacements alike.
    pub added: Vec<ContainerId>,
    /// The one-file Containers those replacements displaced, which the batch
    /// removed (spec: CP-14, PK-12).
    ///
    /// A replacement is a new Container with a new ID rather than the old one
    /// rewritten, so nothing here also appears in [`added`](Self::added): a
    /// removed Container ID is never added again (spec: CP-14, PK-15).
    pub replaced: Vec<ContainerId>,
    /// How many files were found unchanged.
    ///
    /// Both shapes of unchanged: a file whose length and modification time are
    /// what this device last observed, and one that was touched but whose
    /// content still hashes to what its Entry records. The second kind cost a
    /// read and refreshed the local observation, so the next run does not read
    /// it again.
    pub unchanged: usize,
    /// What the run found and did not act on (spec: PK-14).
    pub deferred: Vec<Deferred>,
    /// The mappings whose local roots the device could not vouch for
    /// (spec: EP-12).
    ///
    /// The other half of what a successful run has to be read for. A run that
    /// returns `Ok` with entries here has scanned *less* than the device's
    /// mappings cover, and has deliberately inferred no deletion under them: an
    /// unplugged disk or an unmounted share is reported as an unavailable root
    /// rather than read as the user having emptied the folder (spec: EP-12,
    /// PK-14). The device's other mappings scanned normally, so what the rest of
    /// this outcome says is about them.
    pub unavailable: Vec<UnavailableRoot>,
    /// What this run made of the pending rows an interrupted run left behind
    /// (spec: OC-2).
    ///
    /// Settled before the scan, and settled both ways: a Container no record
    /// names is disposed of (spec: OC-3), and one the caught-up Index says is
    /// current has its bookkeeping completed — an earlier commit whose record
    /// landed and whose Index refresh did not (spec: OC-7).
    pub reconciled: Vec<Reconciled>,
    /// What the commit did, or `None` when the run had nothing to commit.
    pub commit: Option<CommitOutcome>,
}
