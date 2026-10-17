//! Giving Entries of the Library a new Entry Path without touching their
//! content.
//!
//! A rename changes what the catalog calls an Entry and nothing else: its
//! Container, its entry number and the captured path its Container recorded
//! stay as they were (spec: FM-9, FM-15). So the operation writes one Journal
//! record whose `renames` name each Entry by `(Container ID, entry number)` and
//! give it its new path, and never reads, encrypts or uploads an Entry's
//! content or writes a Container — the only Storage writes are the record and
//! what the commit writes for any record (spec: CP-1).
//!
//! The sequence, and the rule each step answers to:
//!
//! 1. **Refuse what needs no catalog.** The same path on both sides, and
//!    either side in the namespace reserved for derived Entries
//!    (spec: FM-15).
//! 2. **Catch up** (spec: CK-9). Which Entries the source names, and what
//!    stands where they would go, are questions about the current Library.
//! 3. **Plan** (spec: EP-5, FM-15). The source names the Entry at that path,
//!    if any, and every current Entry under it — one Entry or a folder of
//!    them — and each is renamed by its own element, the rest of its path kept
//!    under the destination. The record enumerates them: an Entry another
//!    device adds under the folder meanwhile is never swept in (spec: EP-7).
//!    A destination already held by an Entry the record does not rename away,
//!    one that would make a path both a file and a folder, and a folder renamed
//!    under itself are refused, as is anything FM-15's rename-set check
//!    refuses — all before anything is written.
//! 4. **Commit** (spec: CP-1, CP-4). One batch of renames, prepared against
//!    the head the plan read, through [`commit_batch`](crate::commit::commit_batch):
//!    a commit another device made meanwhile is rebased onto, or answered with
//!    [`RebaseConflict`](crate::commit::CommitError::RebaseConflict)
//!    (spec: EP-7).
//! 5. **Relocate this device's files** (spec: EP-10). Where the caller hands
//!    over the device's mapped folders ([`RenameRequest::with_local_disk`]),
//!    the files this device holds of the renamed Entries follow them at once,
//!    as the next sync or fetch would relocate them; a refusal is reported the
//!    way a sync reports one, and the rename stands.
//!
//! What is deliberately not here: noticing that a person renamed a file on
//! disk, which a sync still reads as a deletion and an addition; and derived
//! Entries, which this version does not rename (spec: FM-15).

mod plan;

mod rename_error;
pub use rename_error::{RenameError, RenameResult};

mod rename_outcome;
pub use rename_outcome::RenameOutcome;

mod rename_refusal;
pub use rename_refusal::RenameRefusal;

mod rename_request;
pub use rename_request::RenameRequest;

mod renamed_path;
pub use renamed_path::RenamedPath;

mod run;
pub use run::rename_entries;

// This crate's own tests rather than suite cases, because they count what the
// store was asked to write and read, which is the whole of the claim that a
// rename uploads nothing.
#[cfg(test)]
mod tests;
