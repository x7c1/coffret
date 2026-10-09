//! Taking Entries out of the Library.
//!
//! Until this, the only way an Entry left the Library was by being replaced: a
//! sync's new one-file Container superseding the old, a freeze's Pack absorbing
//! one-file Containers. This is the operation that removes Entries and puts
//! nothing in their place — one invocation, one Journal batch (spec: PK-9,
//! CP-1).
//!
//! The sequence, and the rule each step answers to:
//!
//! 1. **Catch up, and read the committed Keyring** (spec: CK-9, KL-1, KL-7).
//!    Which Containers hold the named Entries, and what else each one holds,
//!    is a question about the current Library. The Keyring is read for the
//!    envelopes that open the Packs to rebuild and for the markers that say
//!    which cannot be opened at all.
//! 2. **Plan** (spec: PK-9). Every Container holding a named Entry is looked
//!    at whole: one whose Entries are all named goes to removals; one that also
//!    keeps Entries is rebuilt around them — unless its key is lost, in which
//!    case nothing can be read back to rebuild from and the deletion is refused
//!    for that Container (spec: PK-10, KL-17). The plan reads the catalog and
//!    nothing else, which is what lets [`preview_delete`] be the same plan.
//! 3. **Rebuild** (spec: PK-10, PK-15). Each Pack that keeps Entries is read
//!    whole, every Entry of it verified, and the kept Entries streamed into a
//!    replacement of the same kind under a new Container ID and Key, with their
//!    recorded metadata and in their original order. One that does not verify
//!    is refused and commits nothing; no replacement is ever written missing a
//!    kept Entry.
//! 4. **Upload** (spec: FM-3), as a freeze uploads its Packs.
//! 5. **Commit** (spec: CP-1, CP-14). One batch: the outright removals and the
//!    replaced Packs in removals, the replacements in additions.
//!    [`commit_batch`](crate::commit::commit_batch) supplies the rest — the
//!    Keyring repair and pre-replication, the Entry Path check, the rebase —
//!    and, after the commit, moves every removed object to the provider's
//!    trash, reporting one that would not go without un-committing anything
//!    (spec: OC-6).
//!
//! Deletion is not undoable in the Library. A removed Container ID is never
//! added again (spec: CP-14), and the provider's trash keeps the removed
//! ciphertext only until it is purged — putting it back from there restores
//! neither its membership in the Library nor its key.
//!
//! [`delete_entries`] is the whole of the public surface that changes
//! anything, and [`preview_delete`] the one place a caller may look in from
//! outside: the plan, counted, with no Storage, no key, and no write.
//! [`committed_key_lost`] is the run's own first step on its own, for a caller
//! that wants the preview to name the Packs a run would be refused for.
//!
//! What is deliberately not here. **The local files** a device holds for a
//! deleted Entry: a deletion touches no mapped folder, and what becomes of a
//! file whose Entry left the Library is the next sync's to decide (spec: EP-15).
//! **`update`** (spec: PK-11, PK-12), which rebuilds Packs the same way and
//! substitutes changed Entries; the rebuild is kept apart from this flow for it.
//! **Repack and compaction** (spec: PK-8), which regroup what deletions leave
//! smaller.

mod delete_error;
pub use delete_error::{DeleteError, DeleteResult};

mod delete_outcome;
pub use delete_outcome::DeleteOutcome;

mod delete_preview;
pub use delete_preview::{preview_delete, DeletePreview};

mod delete_request;
pub use delete_request::DeleteRequest;

mod delete_selection;
pub use delete_selection::DeleteSelection;

mod key_lost;
pub use key_lost::committed_key_lost;

mod pack_refusal;
pub use pack_refusal::PackRefusal;

mod plan;

mod rebuilt_pack;
pub use rebuilt_pack::RebuiltPack;

mod refused_pack;
pub use refused_pack::RefusedPack;

mod run;
pub use run::delete_entries;

// The verdict a rebuild reaches about a Container that did not verify belongs to
// read-modify-replace rather than to deletion — `update` will reach the same
// one — so it is named once at the crate root's module and re-exported where a
// caller of this flow already reaches for the rest of its words. The keys are
// shared with the other flows for the reason they give.
pub use crate::library_keys::LibraryKeys;
pub use crate::local_operation::LocalOperation;
pub use crate::read_modify_replace::Unverified;
