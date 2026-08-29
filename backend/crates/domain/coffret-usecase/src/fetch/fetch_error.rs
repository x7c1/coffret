use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use coffret_model::{ContainerId, ContentHash, EntryPath};

use crate::commit::CommitError;
use crate::error::Error;
use crate::index_error::IndexError;
use crate::local_operation::LocalOperation;

/// Result alias for the fetch.
pub type FetchResult<T> = std::result::Result<T, FetchError>;

/// Everything a fetch can fail with.
///
/// A vocabulary of its own, for the reason [`SyncError`](crate::sync::SyncError)
/// is one: a fetch fails at things no port ever reports — an object that is not
/// the ciphertext the catalog names, a Container that decodes to content the
/// current Entry does not describe, an Entry Path this device cannot
/// materialize under its own mappings — and folding those into a port's error
/// type would make that type answer questions it was never asked.
///
/// What the layers below report travels unchanged inside [`FetchError::Storage`],
/// [`FetchError::Index`], [`FetchError::Format`], and [`FetchError::Commit`].
/// The commit's own vocabulary is wrapped rather than flattened, exactly as the
/// sync wraps it: a fetch starts by catching its Index up (spec: CK-9) and reads
/// the committed Keyring the caught-up checkpoint names (spec: KL-1), both of
/// which are the commit flow's routines and fail in its words. Re-drawing those
/// distinctions here would give one verdict two spellings.
///
/// The three integrity verdicts are separate on purpose, because they are three
/// different accusations. [`CiphertextMismatch`](Self::CiphertextMismatch) says
/// the bytes that arrived are not the bytes the record hashed (spec: FM-15) —
/// transfer damage or substitution, and the one of the three that involves no
/// key, which is why a whole-Container fetch reports it in preference to
/// whatever the decode made of the same bytes.
/// [`Format`](Self::Format) carries the format layer's refusal to authenticate
/// what did arrive. [`ContentMismatch`](Self::ContentMismatch) says the object
/// is a genuine Container that does not hold the content this catalog says it
/// holds. Nothing is placed in any of the three cases (spec: EP-11).
///
/// There is deliberately no `PartialEq`: a caller decides from the variant and
/// the fields it names, never by comparing two errors.
#[derive(Debug)]
pub enum FetchError {
    /// Storage failed, or answered something the run cannot go on from.
    Storage(Error),
    /// The Index could not be read or written.
    Index(IndexError),
    /// A Container could not be opened, or a key could not be unwrapped.
    ///
    /// Authentication happens per chunk inside the decode, so a refusal here is
    /// also the answer to "did anything unverified escape": nothing did
    /// (spec: FM-5, FM-8).
    Format(coffret_format::Error),
    /// The catch-up, or the read of the committed Keyring, did not come through.
    ///
    /// A fetch that cannot read the Library's head serves nothing rather than
    /// serving a stale catalog (spec: CK-9), and one that can read no replica of
    /// the Keyring its checkpoint names has met the loss RV-7 describes rather
    /// than the degraded set RV-2 reads through.
    Commit(CommitError),
    /// A local file or directory could not be made, written, stamped, renamed,
    /// or removed.
    ///
    /// The path is in the value and not in the message, for the reason
    /// [`UnrepresentablePath`](crate::IndexError::UnrepresentablePath) keeps one
    /// there: a local path is one of the things that may never reach a log line,
    /// and an error's message is the part most likely to be logged verbatim.
    Io {
        /// What the run was doing.
        operation: LocalOperation,
        /// The file or directory it was doing it to.
        path: PathBuf,
        /// What the operating system reported.
        cause: io::Error,
    },
    /// An Entry Path a mapping does reach and this device still cannot
    /// materialize.
    ///
    /// Not the general case of
    /// [`UnmappedEntryPath`](Self::UnmappedEntryPath): that one is a path no
    /// mapping reaches at all, which is a fact about this device rather than
    /// about the path.
    ///
    /// Either the path is not one EP-2 admits — an empty, `.`, or `..`
    /// component, a leading or trailing `/`, or a NUL — or it is exactly the
    /// prefix one of this device's mappings stands for, which would make the
    /// local root itself the file. Reported rather than sanitized: coffret never
    /// invents a different local name for an Entry, and a path that could climb
    /// out of a mapped folder is never followed (spec: EP-2, EP-4).
    UnmaterializablePath {
        /// The path that cannot be materialized.
        path: EntryPath,
    },
    /// Two Entry Paths would be materialized at one local path.
    ///
    /// A device whose local roots nest, or whose filesystem cannot tell two
    /// Entry Paths apart, cannot hold both files. Neither is placed and nothing
    /// is renamed: this is EP-4's compatibility error, from the placing side.
    LocalPathCollision {
        /// The path reached first.
        first: EntryPath,
        /// The path that would land on top of it.
        second: EntryPath,
    },
    /// A current Container has no handle Storage will accept.
    ///
    /// The Index caches one per Container it uploaded or fetched, and the walk
    /// the catch-up made answers for the rest by name (spec: FM-3). A Container
    /// neither knows about is one the Library says is current and Storage does
    /// not hold.
    ContainerUnreachable {
        /// The Container that could not be reached.
        container_id: ContainerId,
    },
    /// The ciphertext that arrived is not the ciphertext the record hashed.
    ///
    /// A claim about the whole object, so it is settled once the last byte has
    /// passed rather than as the object arrives, and a decode that failed on the
    /// way is held until it has been: a substituted or damaged object is
    /// reported as that rather than as a Container that would not open
    /// (spec: FM-15, CP-11). Nothing is placed.
    ///
    /// Only [`fetch_folders`](super::fetch_folders) raises it. A range read
    /// deliberately does not ask for the rest of the object, so it has no
    /// standing to make this claim at all (spec: PK-16).
    CiphertextMismatch {
        /// The Container whose object did not arrive as the record describes it.
        container_id: ContainerId,
        /// The hash the Journal record recorded for it.
        expected: ContentHash,
        /// The hash of the bytes that arrived.
        actual: ContentHash,
    },
    /// A Container the catalog says holds an Entry does not hold it.
    ///
    /// The entry table a record carried is what the Index answers from
    /// (spec: CP-11), so an authentic Container whose own table disagrees with
    /// it means the two describe different states of the Library.
    EntryMissing {
        /// The Container that was opened.
        container_id: ContainerId,
        /// The Entry Path the catalog places inside it.
        path: EntryPath,
    },
    /// An Entry's plaintext is not the content the current catalog names.
    ///
    /// Authenticity proves the bytes are a coffret object sealed under the key
    /// that opens this Container; this comparison is the other half, and proves
    /// they are the committed content the catalog stands for (spec: FM-9,
    /// CP-11). Nothing is placed (spec: EP-11).
    ContentMismatch {
        /// The Container the Entry was decoded out of.
        container_id: ContainerId,
        /// The Entry Path whose content did not match.
        path: EntryPath,
    },
    /// The committed Keyring says nothing at all about a current Container.
    ///
    /// At every commit boundary it maps every current Container, to an envelope
    /// or to an explicit key-lost marker (spec: KL-7). A Container it maps to
    /// neither is a control state a fetch cannot act on: a missing entry is not
    /// a key-lost marker, and treating it as one would report a loss the Library
    /// never recorded.
    UnmappedContainer {
        /// The Container the committed Keyring says nothing about.
        container_id: ContainerId,
    },
    /// The Library holds no current Entry at the path a partial fetch named.
    ///
    /// Only [`fetch_entry`](super::fetch_entry) raises it, and only because that
    /// call is about one Entry a caller named: a folder fetch places what the
    /// Library currently holds and has nothing to say about a path it holds
    /// nothing at. Raised after the catch-up, so it is an answer about the
    /// Library's head rather than about a stale catalog (spec: CK-9, EP-5).
    EntryNotCurrent {
        /// The path the Library holds no current Entry at.
        path: EntryPath,
    },
    /// No mapping says where a current Entry's file would go on this device.
    ///
    /// A mapping is what makes a local path exist at all, so an Entry outside
    /// every one of them is outside what this device can materialize rather than
    /// something to invent a place for (spec: EP-9).
    ///
    /// The path itself is unremarkable — a folder fetch would pass over it in
    /// silence, having nothing to place it into. What makes it a verdict is that
    /// a caller named this one Entry. A path a mapping *does* reach and that
    /// still cannot become a file is
    /// [`UnmaterializablePath`](Self::UnmaterializablePath).
    UnmappedEntryPath {
        /// The path no mapping covers.
        path: EntryPath,
    },
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(f, "{error}"),
            Self::Index(error) => write!(f, "{error}"),
            Self::Format(error) => write!(f, "{error}"),
            Self::Commit(error) => write!(f, "{error}"),
            // The path stays out of the message, and stays in the value: see
            // the variant.
            Self::Io {
                operation, cause, ..
            } => write!(
                f,
                "a local file or folder could not be {operation}: {cause}"
            ),
            // An Entry Path is what identifies each of the next two, so the
            // message carries it — and the fetch therefore logs nothing about
            // them, because an Entry Path never belongs in a log line.
            Self::UnmaterializablePath { path } => write!(
                f,
                "the Entry Path {:?} cannot be materialized under this device's mappings",
                path.as_str()
            ),
            Self::LocalPathCollision { first, second } => write!(
                f,
                "the Entry Paths {:?} and {:?} would be materialized at one local path",
                first.as_str(),
                second.as_str()
            ),
            Self::ContainerUnreachable { container_id } => write!(
                f,
                "Container {container_id} is current and Storage holds no object for it"
            ),
            Self::CiphertextMismatch {
                container_id,
                expected,
                actual,
            } => write!(
                f,
                "the object fetched for Container {container_id} hashes to {}, \
                 and its record names {}",
                hex(actual),
                hex(expected),
            ),
            Self::EntryMissing { container_id, path } => write!(
                f,
                "Container {container_id} does not hold the Entry the catalog places in it, {:?}",
                path.as_str()
            ),
            Self::ContentMismatch { container_id, path } => write!(
                f,
                "the Entry {:?} in Container {container_id} is not the content the catalog names",
                path.as_str()
            ),
            Self::UnmappedContainer { container_id } => write!(
                f,
                "the committed Keyring holds neither an envelope nor a key-lost \
                 marker for Container {container_id}"
            ),
            // An Entry Path is what identifies each of these two, so the message
            // carries it — and the fetch therefore logs nothing about them.
            Self::EntryNotCurrent { path } => write!(
                f,
                "the Library holds no current Entry at {:?}",
                path.as_str()
            ),
            Self::UnmappedEntryPath { path } => write!(
                f,
                "no mapping of this device says where the Entry at {:?} would go",
                path.as_str()
            ),
        }
    }
}

impl error::Error for FetchError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            Self::Index(error) => Some(error),
            Self::Format(error) => Some(error),
            Self::Commit(error) => Some(error),
            Self::Io { cause, .. } => Some(cause),
            Self::UnmaterializablePath { .. }
            | Self::LocalPathCollision { .. }
            | Self::ContainerUnreachable { .. }
            | Self::CiphertextMismatch { .. }
            | Self::EntryMissing { .. }
            | Self::ContentMismatch { .. }
            | Self::UnmappedContainer { .. }
            | Self::EntryNotCurrent { .. }
            | Self::UnmappedEntryPath { .. } => None,
        }
    }
}

impl From<Error> for FetchError {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}

impl From<IndexError> for FetchError {
    fn from(error: IndexError) -> Self {
        Self::Index(error)
    }
}

impl From<coffret_format::Error> for FetchError {
    fn from(error: coffret_format::Error) -> Self {
        Self::Format(error)
    }
}

impl From<CommitError> for FetchError {
    fn from(error: CommitError) -> Self {
        Self::Commit(error)
    }
}

/// One content hash as the lowercase hex a message and an object name spell it
/// in (spec: FM-12).
fn hex(hash: &ContentHash) -> String {
    hash.as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
