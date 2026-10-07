use std::error;
use std::path::PathBuf;

use coffret_model::{ContainerId, EntryPath};

/// Result alias for [`Index`](crate::Index) operations.
pub type IndexResult<T> = std::result::Result<T, IndexError>;

/// Everything an [`Index`](crate::Index) operation can fail with.
///
/// It is a vocabulary of its own rather than the Storage port's
/// [`Error`](crate::Error), because the two ports fail at different things: the
/// Index is a device-local catalog that no provider is involved in, so nothing
/// here is a lost race, a throttle, or a transport fault, and nothing here is
/// worth retrying unchanged.
///
/// The catalog being a cache and never the source of truth (spec: RV-5) is what
/// makes the failures here small: whatever cannot be read back can be rebuilt
/// from Storage, so the type says what the caller must do — rebuild, resolve a
/// conflict, install a build that knows the file, or name the local path or the
/// number the catalog cannot keep — rather than describing a backend.
#[derive(Debug)]
pub enum IndexError {
    /// Another sync or freeze owns this device's pending rows. Retry after it
    /// finishes; its in-flight ciphertext must not be reclaimed (spec: OC-2).
    PendingRowsBusy {
        /// The lock implementation's refusal, kept for inspection.
        cause: Box<dyn error::Error + Send + Sync>,
    },
    /// The Index has never been given a committed state to stand on.
    ///
    /// A fresh Index catalogs nothing and checkpoints nothing until a
    /// [`restore`](crate::Index::restore) adopts a Snapshot or an
    /// [`apply`](crate::Index::apply) replays a record, so there is no content
    /// to hand back and no checkpoint to write into one (spec: CK-9).
    NoCheckpoint,
    /// Two Entries would occupy one Entry Path.
    ///
    /// At every committed Library state one Entry Path identifies at most one
    /// current Entry, so a replay or a restore that would place a second one
    /// there is describing a state no commit could have produced (spec: EP-5,
    /// EP-6).
    DuplicatePath {
        /// The path claimed twice.
        path: EntryPath,
    },
    /// One Container would be added to the current set twice.
    DuplicateContainer {
        /// The Container claimed twice.
        container_id: ContainerId,
    },
    /// An Entry names a Container the current set does not hold.
    ///
    /// A record carries the Entries of the Containers it adds, so an Entry
    /// without its Container is a record or a Snapshot that cannot be replayed
    /// as it stands (spec: CP-11).
    UnknownContainer {
        /// The Container the Entry names.
        container_id: ContainerId,
    },
    /// A local path the catalog has no way to keep.
    ///
    /// A filesystem may hand out a name that is not valid UTF-8, and a catalog
    /// that keeps paths as text cannot store one without changing it. Keeping a
    /// lossy spelling would point a mapping or a spool at a file that is not the
    /// one meant, so the path is refused instead.
    ///
    /// The path travels in the value so a caller can say which file it is
    /// about; the message leaves it out, because a local path is the user's own
    /// and a message may end up wherever an error is reported.
    UnrepresentablePath {
        /// What the Index was doing.
        operation: &'static str,
        /// The path that cannot be kept.
        path: PathBuf,
    },
    /// A number the catalog has no column wide enough to hold.
    ///
    /// The counterpart of [`IndexError::UnrepresentablePath`] for the numbers a
    /// catalog keeps. Every number that comes out of the format is already
    /// below 2^63 — that is the bound FM-19 puts on them, and the types that
    /// carry them refuse a larger one at their constructors — so an offset, a
    /// size, an epoch, and a generation never reach this refusal at all.
    ///
    /// What is left is the one number the format does not bound: the length
    /// this device's own filesystem reported for a local file
    /// ([`LocalObservation`](crate::device_state::LocalObservation)'s `size`).
    /// A file the filesystem calls 2^63 bytes or more is not one this device
    /// can record having, and being told so is the honest answer — where
    /// storing the same bits under a sign they do not have would leave the
    /// catalog reading back a value nothing ever wrote.
    ///
    /// The number travels in the message: a file's length is arithmetic, and
    /// the column beside it says which number it was, where the local path it
    /// belongs to stays out.
    UnrepresentableValue {
        /// What the Index was doing.
        operation: &'static str,
        /// The column the value was headed for.
        column: &'static str,
        /// The value that has no spelling there.
        value: u64,
    },
    /// The Index file is laid out in a way this build cannot open, and cannot
    /// repair by discarding the catalog alone.
    ///
    /// The catalog being a cache is what lets an adapter throw one away and
    /// rebuild it from Storage (spec: RV-5), so an older layout is not
    /// ordinarily refused at all. This is the case where that is not enough:
    /// beside the catalog the file holds the state that is only ever this
    /// device's — where the Library is mapped onto its folders, what it has on
    /// disk, what it spooled and never committed (spec: EP-9, EP-10, OC-2) —
    /// and no adapter can keep that across a layout it does not read, nor
    /// recover it from anywhere else. A file from a *newer* build is refused
    /// for the plainer reason that this one cannot read any of it.
    ///
    /// So the answer is the owner's rather than the adapter's, and the message
    /// states it: the mappings can still be read out of the file before it
    /// goes, so delete it, record them again, and catch up.
    UnsupportedSchema {
        /// The version found in the file.
        found: i64,
        /// The version this build writes and reads.
        supported: i64,
    },
    /// The catalog holds a value this build cannot read back.
    ///
    /// A Container kind spelled in a vocabulary this build has no reading for,
    /// a stored digest the domain does not admit, half a reference where a
    /// whole one belongs: the file was written by something else, or damaged.
    /// The answer is the one [`IndexError::UnsupportedSchema`] states — the
    /// file cannot be carried forward, so it goes and the catalog is rebuilt
    /// from Storage (spec: RV-5) — and not the one a store that merely failed
    /// asks for, which is why the two are separate.
    UnreadableCatalog {
        /// What the Index was doing.
        operation: &'static str,
        /// What could not be read, as whatever refused it reported.
        cause: Box<dyn error::Error + Send + Sync>,
    },
    /// The store the catalog is kept in failed.
    ///
    /// The store itself, not what it holds: a file that cannot be opened, a
    /// write that cannot be carried out, a thread that never finished.
    Backend {
        /// What the Index was doing.
        operation: &'static str,
        /// What the store reported.
        cause: Box<dyn error::Error + Send + Sync>,
    },
}

mod display;
mod redacted;
mod source;

#[cfg(test)]
mod tests;
