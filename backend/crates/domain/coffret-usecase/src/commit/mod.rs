//! Making a prepared batch the Library's next committed state.
//!
//! Everything the commit path needs exists elsewhere — the control payload
//! forms in `coffret-format`, the two ports in this crate,
//! [`ControlHead`](crate::ControlHead) over the storage one — and this module is
//! what composes them. It takes a batch whose Containers are already on Storage
//! and carries it through the sequence the commit protocol defines:
//!
//! 1. **Catch up.** Bring the Index to the current head from the newer of two
//!    starting points, its own state or the newest valid checkpoint, and replay
//!    the Journal after it (spec: CK-9). The same routine is what a conflict
//!    rebase runs, which is why it is one step rather than a preamble.
//! 2. **Check the candidate.** Every Container the batch removes has to still
//!    be current, and the post-commit Entry set has to satisfy the Entry Path
//!    uniqueness a commit rests on; a batch that fails either is refused before
//!    anything is written (spec: CP-18, EP-6).
//! 3. **Examine and repair the committed Keyring.** Read every position the
//!    committed commitment declares, rewrite the ones that are absent or do not
//!    read back valid from one that does, and confirm each rewrite by reading it
//!    back. A committed set that has lost replicas must be complete again before
//!    another write, and a repair that cannot complete refuses the commit
//!    outright (spec: KL-11, KL-13, KL-14, KL-16).
//! 4. **Pre-replicate the Keyring.** Build the next generation over exactly the
//!    post-commit Container set, write every replica, and read every one of
//!    them back: no commit happens until that candidate set is complete
//!    (spec: CP-8, CP-9, KL-2, KL-14).
//! 5. **Commit.** Reserve the two slots the new record carries, re-read the
//!    head, and consume the commit slot that head carries with the Journal
//!    record. Creating that object is the batch's commit point
//!    (spec: CP-1, CP-2, CP-3, CP-15, CP-16).
//! 6. **Rebase on a conflict.** A consumed slot is a normal outcome, not a
//!    failure: the flow catches up onto the new head and starts again, capped at
//!    [`CommitPolicy::attempts`] (spec: CP-4, CP-7). Every attempt carries the
//!    batch over the records committed since the head it was prepared against
//!    before step 2: their renames are applied to its pending changes by
//!    `(Container ID, entry number)`, and a write to one Entry or Entry Path
//!    from two sides is refused as a conflict (spec: EP-7).
//! 7. **After the commit.** Refresh the Index with the batch, trash what the
//!    batch removed, and write the checkpoint if the policy asks for one
//!    (spec: CK-8, CK-10, CK-11).
//!
//! [`commit_batch`] is the whole of the public surface, and the steps that write
//! are private because none of them is a state a caller may stop at — a Keyring
//! candidate without its commit is exactly the uncommitted set KL-3 says selects
//! nothing. Two steps are the exception, and only within this crate: the
//! catch-up, which writes nothing to the Library and leaves the Index standing
//! at the head — precisely where a run settling its own pending rows stops
//! (spec: OC-3, OC-7), and where [`catch_up`](crate::catch_up) stops for a
//! caller that wants the head read and nothing brought over; and reading the
//! committed Keyring, which is the KL-1
//! replica walk a [`fetch`](crate::fetch) makes for the envelopes that open what
//! it fetched (spec: KL-7, RV-3). Neither is the commit's alone, and a second
//! copy of either would be a second reading of the rule it answers.
//!
//! What is deliberately not here: producing the batch (scanning, packing,
//! encrypting, uploading Containers, and a deletion's rebuilds — see
//! [`delete`](crate::delete)); `prune`, which [`prune`](crate::prune) makes
//! over the checkpoint this module writes; and Master Key epoch activation. An
//! activation met while replaying is reported as
//! [`CommitError::EpochActivated`] rather than handled (spec: CP-5).

#[cfg(test)]
mod adversarial_store_tests;

mod after_commit;
// The checkpoint of one head, for `prune` to rest on: the Snapshot already in
// the head's slot, or the Index written there (spec: CK-8, CK-10, CK-11).
pub(crate) use after_commit::{checkpoint_head, Checkpointed};

mod candidate;

mod catch_up;
pub(crate) use catch_up::{catch_up, read_records, CaughtUp};

#[cfg(test)]
mod catch_up_tests;
#[cfg(test)]
mod snapshot_schema_tests;

mod checkpoint_failure;
pub use checkpoint_failure::CheckpointFailure;

mod checkpoint_outcome;
pub use checkpoint_outcome::CheckpointOutcome;

mod commit_error;
pub use commit_error::{
    CommitError, CommitResult, ControlObjectFault, UnrepairedReplica, UnusableReplica,
};

mod commit_failure;
pub use commit_failure::CommitFailure;

mod commit_outcome;
pub use commit_outcome::CommitOutcome;

mod commit_policy;
pub use commit_policy::CommitPolicy;

mod commit_request;
pub use commit_request::CommitRequest;

// How far one attempt has got, in the objects it stores — reported as each
// replica of the candidate lands and once the record has (spec: CP-8, CP-2).
mod committing;

#[cfg(test)]
mod control_fixtures;

mod control_keys;
pub use control_keys::ControlKeys;

mod control_listing;
// The walk of Storage a caught-up Index came from. It travels with the Index,
// because the handles in it are how a Container or a Keyring replica this device
// never wrote is reached at all (spec: FM-3, FM-12).
pub(crate) use control_listing::ControlListing;

mod control_object;
// What kind of control object stands at a name, read off its plaintext header
// alone: one name form covers a Journal record and an activation Snapshot
// alike, and `prune` removes only the first (spec: CK-6, FM-12).
pub(crate) use control_object::fetch_header;

// The name a rename leaves an Entry, which a batch's pending changes act on.
// It is the crate's rather than the commit's, because a materialization record
// names the Entry it is a copy of the same way (spec: FM-9, EP-10).
pub use crate::entry_ref::EntryRef;

// A Library's first head, written when the Library is created so that no
// commit ever starts from no head (spec: CP-2, FM-13).
mod first_head;
pub use first_head::write_first_head;

#[cfg(test)]
mod first_head_tests;

mod journal;

mod keyring;
// Reading the committed Keyring, for whoever needs the envelopes it maps. A
// fetch does: it opens the Containers it pulled back with them (spec: KL-7,
// RV-3), and a freeze asks the same mapping which Containers have no key. The
// read is the KL-1 replica walk, over the same listing the catch-up already
// took — the reading half of the walk a commit makes before it repairs the set
// (spec: KL-11, KL-13). The two beside it are what that read hands back where
// it had to step over a position: the finding itself — public, because a run
// that reached no examination hands it to its caller on its outcome — and the
// guard a caller that goes on to write holds it in until either its own
// exit or the commit's examination says it, so that one run says one word about
// one set (spec: KL-5, KL-15).
pub use keyring::ShortKeyring;
pub(crate) use keyring::{read_committed, shortfall, ShortReport};

mod keyring_repair;
pub use keyring_repair::KeyringRepair;

// Reading back a commit slot a batch recorded, which is how settling proves a
// batch of unknown outcome did not commit (spec: OC-3).
mod slot_occupant;
pub(crate) use slot_occupant::read as read_slot_occupant;
pub(crate) use slot_occupant::SlotOccupant;

mod prepared_addition;
pub use prepared_addition::PreparedAddition;

mod prepared_batch;
pub use prepared_batch::PreparedBatch;

mod pending_change;
pub use pending_change::PendingChange;

mod pending_rename;
pub use pending_rename::PendingRename;

// Carrying a batch over the records committed since it was prepared: their
// renames applied to its pending changes, and the conflicts they cause
// (spec: CP-4, EP-7).
mod rebase;

mod rebase_rule;
pub use rebase_rule::RebaseRule;

mod rewritten_replicas;
pub use rewritten_replicas::RewrittenReplicas;

mod pending_provenance;
mod run;
pub use run::commit_batch;

mod untrashed_removal;
pub use untrashed_removal::UntrashedRemoval;
