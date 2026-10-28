//! Removing the Journal records an Index Snapshot covers.
//!
//! Every commit adds one Journal record and nothing else ever takes one away,
//! so without this a Library's Journal holds every sync, freeze, rename and
//! deletion it has ever made, and a device recovering it from nothing reads all
//! of them before it can answer anything. A checkpoint already stands in for the
//! records it applies — what they did to the Catalog, the Library's name, the
//! slot the next commit goes into, the Keyring tuple they selected (spec: CK-1,
//! CK-2, CK-3, CK-7) — and `prune` is what lets it replace them.
//!
//! The sequence, and the rule each step answers to:
//!
//! 1. **Catch up** (spec: CK-9). The head is what the Snapshot below has to
//!    checkpoint, and it is a question about the current Library.
//! 2. **Checkpoint first** (spec: CK-8, CK-10, CK-11). The ordinary Snapshot of
//!    the head is made sure of: the one in the head's `snapshot_slot` where
//!    another device already wrote it, and otherwise this device's Index,
//!    written there. It is what the rest rests on, and it stands whatever the
//!    later steps decide.
//! 3. **Gate** (spec: CK-5). That Snapshot must record the exact Keyring tuple
//!    the head committed (spec: CK-3), and that tuple's replica set must be
//!    complete on Storage (spec: KL-2): once the records are gone, the replicas
//!    are the envelopes' only carriers and the Snapshot's tuple the only proof
//!    of which set was selected. Either failing refuses the run with
//!    [`PruneRefusal`] before anything is deleted.
//! 4. **Delete** (spec: CK-4, CK-6). The Journal records at or before the
//!    Snapshot's last applied Journal generation go — every one but the
//!    current head — through the store's recoverable
//!    [`trash`](crate::ObjectStore::trash), as a commit's removed Containers
//!    do. Nothing else on Storage is touched.
//!
//! What it answers with is [`PruneOutcome`]: the Snapshot it rested on, and
//! the generations it removed.
//!
//! # The current head stays
//!
//! CK-4 makes the head itself eligible, and this implementation keeps it: a
//! writer on another device may have read the head a moment ago and be about
//! to consume its slot, and no ordinary commit exercises a head chain whose
//! newest link is a Snapshot. The note under CK-6 gives the whole reason.
//!
//! # Running beside other devices
//!
//! A record already gone when its turn comes — another device pruned the same
//! stretch a moment before — is not a failure: it is where the run was taking
//! it. A record Storage will not trash is reported in the outcome and the run
//! goes on, and what it did remove stays removed, so running `prune` again
//! finishes the job.
//!
//! The other side of that race is not retried within the run. A device
//! part-way through replaying the stretch a prune removes, from a listing it
//! took before the Snapshot existed, meets the gap as
//! [`MissingHead`](crate::commit::CommitError::MissingHead) and its run fails;
//! so does a commit whose batch was prepared against a head the prune removed,
//! when it reads the records it has to rebase over (spec: CP-4), and nothing of
//! that batch is committed. Running it again is the remedy: the next catch-up
//! lists the Snapshot, which is newer than anything the device stood on, and
//! starts from it (spec: CK-9).
//!
//! # What it deliberately does not do
//!
//! It never runs on its own. Nothing calls it after a commit or a checkpoint:
//! whether and when a device prunes without being asked is a policy choice,
//! like CK-8's threshold, and this is the operation a policy would call rather
//! than the policy. It removes no Index Snapshot — not the one it rests on,
//! and not an older ordinary one a newer Snapshot supersedes, since no rule
//! says when one of those may go — and no Keyring replica of any generation,
//! and no Container.

mod prune_error;
pub use prune_error::{PruneError, PruneResult};

mod prune_outcome;
pub use prune_outcome::PruneOutcome;

mod prune_refusal;
pub use prune_refusal::PruneRefusal;

mod prune_request;
pub use prune_request::PruneRequest;

mod run;
pub use run::prune_journal;

mod unremoved_record;
pub use unremoved_record::UnremovedRecord;

#[cfg(test)]
mod tests;
