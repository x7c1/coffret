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
//! 2. **Check the candidate.** The post-commit Entry set has to satisfy the
//!    Entry Path uniqueness a commit rests on, and a batch that would break it
//!    is refused before anything is written (spec: EP-6).
//! 3. **Pre-replicate the Keyring.** Build the next generation over exactly the
//!    post-commit Container set, write every replica, and read every one of
//!    them back: no commit happens until that candidate set is complete
//!    (spec: CP-8, CP-9, KL-2, KL-14).
//! 4. **Commit.** Reserve the slots, re-read the head, and spend the commit slot
//!    on the Journal record. Creating that object is the batch's commit point
//!    (spec: CP-1, CP-2, CP-3, CP-16).
//! 5. **Rebase on a conflict.** A consumed slot is a normal outcome, not a
//!    failure: the flow catches up onto the new head and starts again, capped at
//!    [`CommitPolicy::attempts`] (spec: CP-4, CP-7).
//! 6. **Settle.** Refresh the Index with the batch, trash what the batch
//!    removed, and write the checkpoint if the policy asks for one (spec: CK-8,
//!    CK-10, CK-11).
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
//! encrypting, uploading Containers), the removals-only deletion flow, `prune`,
//! and Master Key epoch activation. An activation met while replaying is
//! reported as [`CommitError::EpochActivated`] rather than handled (spec: CP-5).

mod candidate;

mod catch_up;
pub(crate) use catch_up::catch_up;

mod checkpoint_outcome;
pub use checkpoint_outcome::CheckpointOutcome;

mod commit_error;
pub use commit_error::{CommitError, CommitResult, ControlObjectFault, InvalidReplica};

mod commit_outcome;
pub use commit_outcome::CommitOutcome;

mod commit_policy;
pub use commit_policy::CommitPolicy;

mod commit_request;
pub use commit_request::CommitRequest;

mod control_keys;
pub use control_keys::ControlKeys;

mod control_listing;
// The walk of Storage a caught-up Index came from. It travels with the Index,
// because the handles in it are how a Container or a Keyring replica this device
// never wrote is reached at all (spec: FM-3, FM-12).
pub(crate) use control_listing::ControlListing;

mod control_object;

mod journal;

mod keyring;
// Reading the committed Keyring, for whoever needs the envelopes it maps. A
// fetch does: it opens the Containers it pulled back with them (spec: KL-7,
// RV-3), and the read is the same KL-1 replica walk a commit makes to carry the
// generation forward, over the same listing the catch-up already took.
pub(crate) use keyring::read_committed;

mod prepared_addition;
pub use prepared_addition::PreparedAddition;

mod prepared_batch;
pub use prepared_batch::PreparedBatch;

mod run;
pub use run::commit_batch;

mod settle;

mod untrashed_removal;
pub use untrashed_removal::UntrashedRemoval;
