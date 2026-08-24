use coffret_model::ContainerId;

use crate::commit::CommitOutcome;
use crate::sync::deferred::Deferred;
use crate::sync::reconciled::Reconciled;

/// What one sync run found and what it did about it.
///
/// Two halves, and the second is the one that matters most. [`commit`] says
/// what became of the Library, and a run that found nothing to upload carries
/// `None` there rather than an empty commit — a Journal record for a batch that
/// changes nothing is a generation spent on nothing (spec: CP-1). [`deferred`]
/// says what the run left alone, and it is not an afterthought: a scan
/// selecting update candidates has to surface every file that needs one, so a
/// caller reads this list rather than assuming that a successful sync means
/// every local file is backed up (spec: PK-14).
///
/// [`commit`]: Self::commit
/// [`deferred`]: Self::deferred
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
    /// The abandoned spools of interrupted runs this one disposed of
    /// (spec: OC-2).
    ///
    /// A run that committed nothing never read the Library's head, so it leaves
    /// an interrupted run's *uploaded* Container to a later run that does,
    /// rather than deciding from a possibly stale Index that no record names it
    /// (spec: CK-9, OC-3). Those are absent from this list rather than reported
    /// as untrashed: nothing about them was settled.
    pub reconciled: Vec<Reconciled>,
    /// What the commit did, or `None` when the run had nothing to commit.
    pub commit: Option<CommitOutcome>,
}
