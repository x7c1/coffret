//! Regrouping a folder's Entries into Packs cut in Entry Path order.
//!
//! A `freeze` groups Entries by Entry Path only among what one invocation
//! selected, and a rename leaves an Entry in the Pack it was frozen into
//! (spec: PK-8, FM-15). So over time a folder's books come to be spread across
//! Packs that also hold other folders' books, with path ranges that overlap
//! and interleave: reading ahead into the next volume asks for more parcels
//! than it would, and a deletion's read-modify-replace reads more Entries that
//! have nothing to do with it (spec: PK-10, PK-16). This is the operation PK-8
//! names as the remedy — one invocation, over one folder, one Journal batch
//! (spec: PK-23).
//!
//! The sequence, and the rule each step answers to:
//!
//! 1. **Catch up, and read the committed Keyring** (spec: CK-9, KL-1, KL-7).
//!    Which Packs hold the folder's Entries, and at which paths those stand
//!    now, is a question about the current Library. The Keyring is read for
//!    the envelopes that open the Packs to rewrite and for the markers that
//!    say which cannot be opened at all.
//! 2. **Select** (spec: PK-23). Every current Pack holding at least one Entry
//!    under the folder, whole: a Pack is replaced whole, so its Entries
//!    outside the folder take part too. One-file Containers are not inputs —
//!    absorbing those is a freeze's job (spec: PK-1, PK-7).
//! 3. **Cut** (spec: PK-3, PK-4, PK-6). The selected Entries sorted by their
//!    current Entry Path and cut by the freeze's own segmentation, under the
//!    same size target.
//! 4. **Keep what is already in place** (spec: PK-23). A cut segment holding
//!    exactly one selected Pack's Entries, in that Pack's order, each captured
//!    under the path it stands at now, is that Pack: it is neither rewritten
//!    nor removed. A folder already in order commits nothing, and so does a
//!    second run straight after the first.
//! 5. **Refuse before reading** (spec: KL-17, PK-23). A Pack to be rewritten
//!    whose key is lost cannot be read, so the run commits nothing and names
//!    it. Nothing has been read or written by then.
//! 6. **Stage** (spec: PK-10). Each Pack to be rewritten is read whole and
//!    every Entry of it verified by read-modify-replace's own rebuild — the
//!    one a deletion uses — carrying all of them into a copy on this device's
//!    spool, under a key of its own. A new Pack draws its Entries in path
//!    order, which interleaves the old Packs and can run backwards through one
//!    of them where a rename moved an Entry, so the old objects cannot simply
//!    be streamed into the new ones; the copies are what is read instead,
//!    forward, one chunk of plaintext at a time. They are
//!    ciphertext like any spool, and each has a pending row from before its
//!    first byte (spec: OC-2), so whatever stops the run leaves nothing the
//!    next sync's settlement does not dispose of. A Pack that does not verify
//!    commits nothing at all: a repack done in part would leave the order half
//!    made while looking done, and running it again once the cause is fixed
//!    does the whole job.
//! 7. **Write** (spec: PK-10, PK-15, PK-18). Each segment that is not kept is
//!    written into a new Pack under a new Container Key through the writer a
//!    freeze uses, from the staged copies, every Entry carried under its
//!    current Entry Path — which becomes its captured path — with the
//!    modification time, birth time, hash and deposit time it had. The copies
//!    are then disposed of with their rows.
//! 8. **Upload, and commit once** (spec: CP-1, CP-14, OC-6). One batch: the
//!    replaced Packs in removals, the new ones in additions, through
//!    [`commit_batch`](crate::commit::commit_batch) — which rebases over a
//!    rename another device committed meanwhile by following each new Entry
//!    back to the one it carries, as it does for a deletion's rewrite
//!    (spec: EP-7), and trashes the replaced objects once the commit lands.
//!
//! What it costs is the selected Packs read once from Storage, and their bytes
//! through this device's disk twice: the staged copies and the new spools side
//! by side, until the copies go.
//!
//! [`repack_folder`] is the whole of the public surface. It runs only when
//! asked: nothing repacks after a rename, a deletion or a freeze.

mod plan;

mod repack_error;
pub use repack_error::{RepackError, RepackResult};

mod repack_outcome;
pub use repack_outcome::{RepackOutcome, RepackedPack};

mod repack_refusal;
pub use repack_refusal::RepackRefusal;

mod repack_request;
pub use repack_request::RepackRequest;

mod run;
pub use run::repack_folder;

mod staged;

// Why a Pack could not be read for rewriting is read-modify-replace's verdict,
// named once for the deletion that reaches it first and re-exported where a
// caller of this flow reaches for the rest of its words.
pub use crate::delete::{PackRefusal, Unverified};
pub use crate::library_keys::LibraryKeys;

#[cfg(test)]
mod tests;
