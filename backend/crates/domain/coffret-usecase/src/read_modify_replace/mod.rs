//! Replacing a Container by a new one that carries part of it forward.
//!
//! A Container is immutable: changing what it holds means writing a
//! replacement under a new Container ID and removing the old one in the same
//! Journal batch (spec: CP-1, CP-14). Where the change is to take some Entries
//! out of a Pack and leave the others, the replacement has to be built out of
//! the old Pack's own bytes, because those are the only copy of the kept
//! Entries the Library vouches for — this device may never have held any of
//! them. That is read-modify-replace (spec: PK-10), and this module is the
//! read and the replace; deciding which Entries to keep is the caller's.
//!
//! [`rebuild`](fn@rebuild) reads the old Container whole, verifies it three
//! ways — the object against the hash its record names (spec: FM-15), every
//! chunk against its key (spec: FM-5, FM-8), every Entry's plaintext against
//! the hash its row records (spec: CP-11) — and streams the kept Entries
//! straight into a replacement through the writer a freeze uses, under a
//! Container Key of its own (spec: KD-2, FM-14). The replacement keeps the old
//! Container's kind (spec: PK-15) and each kept Entry's recorded metadata —
//! Entry Path, `original_mtime`, `original_btime`, hash — in the order the old
//! Container held them. What it hands back is a spooled Container ready to
//! upload and commit, superseding the old one.
//!
//! If any Entry does not verify, no replacement is handed back: a replacement
//! is a claim that it carries the old Entries forward, and one written from an
//! object that did not verify, or missing a kept Entry, would be that claim
//! made falsely (spec: PK-10). The refusal is an [`Unverified`] and costs that
//! one Container.
//!
//! It is written for a deletion, and kept apart from it because `update` is the
//! other operation that needs it (spec: PK-12): an update's replacement carries
//! the unchanged Entries forward exactly as this does and substitutes the
//! changed ones from local files, which is one more source for the writer and
//! not a second way of reading the old Container.

mod carrying;

mod rebuild;
pub(crate) use rebuild::rebuild;

mod rebuild_error;
pub(crate) use rebuild_error::RebuildError;

mod rebuilding;
pub(crate) use rebuilding::Rebuilding;

mod replacing;
pub(crate) use replacing::Replacing;

mod unverified;
pub use unverified::Unverified;

/// How much of the old object is held at once — a transfer buffer, for the
/// reason a fetch holds one: the bytes go to the chunk reader as they arrive.
const TRANSFER_BUFFER: usize = 128 * 1024;

#[cfg(test)]
mod tests;
