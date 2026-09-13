use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use coffret_model::{ContainerId, ContentHash, EntryPath, Redacted};

use crate::commit::CommitError;
use crate::descent_error::DescentError;
use crate::error::Error;
use crate::index_error::IndexError;
use crate::local_operation::LocalOperation;
use crate::refused_root::RootRefused;

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
    /// there: a local path is one of the things that may never reach a
    /// diagnostic event, and an error's message is the part most likely to be
    /// logged verbatim.
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
    /// Either the path is exactly the prefix one of this device's mappings
    /// stands for, which would make the local root itself the file, or the
    /// folders it names are not folders *on this device*: a component that is a
    /// symbolic link, or an ordinary file where a folder must be. Reported
    /// rather than sanitized: coffret never invents a different local name for
    /// an Entry (spec: EP-4, EP-11). What EP-2 excludes is not among the ways
    /// here, because an [`EntryPath`] is never in one of those shapes.
    ///
    /// The second of the two is the one that depends on what is on disk rather
    /// than on the path alone, and it is why the same Entry Path can be
    /// materializable on the device that committed it and not here. A folder
    /// fetch meets it while *selecting* and reports it as
    /// [`Surfaced::UnreachablePlace`](super::Surfaced::UnreachablePlace)
    /// instead: the shape of one folder is one Entry's business and the run
    /// places the rest. What reaches this variant is the same fence met where
    /// there is no longer a finding to make of it — mid-write, after the
    /// selection found the place sound, and on the upload route, which places
    /// each of the files its drop was handed. There it is one file's refusal
    /// and not the request's, unlike [`RefusedRoot`](Self::RefusedRoot) below:
    /// the descent below a sound root is this path's own, so the part is
    /// refused and the rest of the drop carries on.
    UnmaterializablePath {
        /// The path that cannot be materialized.
        path: EntryPath,
        /// The folder on this device a descent stopped at, where a descent is
        /// what refused.
        ///
        /// `None` where the path alone is the answer: it stands at exactly a
        /// mapping's prefix, so there is no relative path left to descend and no
        /// folder to name. It reaches a person in the message and never a
        /// diagnostic event, the way an unavailable root's folder does
        /// (spec: EL-1).
        stopped_at: Option<PathBuf>,
    },
    /// An Entry Path carries a component coffret keeps for itself.
    ///
    /// Two reservations and one refusal, because they are one rule seen twice:
    /// a name inside a mapped folder that belongs to coffret rather than to the
    /// person. `.coffret` at any depth is the device's own management area,
    /// which a scan never enters and never reports as content (spec: EP-14);
    /// `.coffret-fetch-…` is the prefix reserved for coffret's scratches, which
    /// a scan steps over (spec: EP-11). A file written under either would sit in
    /// a mapped folder that no sync will ever carry in — visible, the person's
    /// own, and permanently outside the Library — and one written at the
    /// management area's marker would take the root's identity away from it
    /// (spec: EP-13).
    ///
    /// Decided from the path alone, before anything on disk is reached, which is
    /// why it is its own verdict rather than an
    /// [`UnmaterializablePath`](Self::UnmaterializablePath): that one says no
    /// local name can stand for the path, and this says the name is one coffret
    /// has already taken. Saying which is what lets the refusal name the
    /// component that made it so (spec: EP-4).
    ///
    /// The verdict a *single* writer gets. A folder fetch meets the management
    /// area's half while selecting and reports it as
    /// [`Surfaced::ReservedComponent`](super::Surfaced::ReservedComponent)
    /// instead, placing the rest of the run.
    ReservedComponent {
        /// The path that carries it.
        path: EntryPath,
        /// The component of it coffret keeps, which is what a person changes.
        ///
        /// A component of an Entry Path and so the user's own name for part of
        /// their file's place: it reaches them in the message and never a
        /// diagnostic event (spec: EL-1).
        component: String,
    },
    /// The mapped root a file was to go into is not the root the mapping was
    /// recorded against (spec: EP-13).
    ///
    /// A single writer's verdict and not a folder fetch's. What a folder fetch
    /// does with the same refusal is report the mapping once and place the rest
    /// of the Library —
    /// [`FetchOutcome::refused`](super::FetchOutcome::refused) — because the
    /// root is one mapping's business and the device's other mappings are sound
    /// (spec: EP-11's reporting). A caller that asked for one Entry has no
    /// other mapping to go on with, and neither does the upload route, however
    /// many files its drop was handed: every one of them goes through the root
    /// that will not vouch for itself, so the request fails as a whole
    /// (spec: EP-13).
    ///
    /// The root travels in the value the way an unavailable root's folder does,
    /// and never into a diagnostic event; neither does the prefix, an Entry Path
    /// component being no more loggable than a local path (spec: EL-1). The
    /// reason does, naming no path.
    ///
    /// The prefix is what a refusal *names*, all the same, and it is why it is
    /// carried: it says which of the device's mappings will not vouch for
    /// itself to whoever has to record that one again.
    RefusedRoot {
        /// The top-level component the mapping stands for, or `None` for the
        /// Library root.
        prefix: Option<EntryPath>,
        /// The folder on this device the mapping names.
        local_root: PathBuf,
        /// Which of EP-13's cases it was.
        reason: RootRefused,
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

impl FetchError {
    /// What a refused descent into a mapped folder means for one Entry.
    ///
    /// A fence the descent met is
    /// [`UnmaterializablePath`](Self::UnmaterializablePath) and nothing else. It
    /// is the same verdict a `..` component or a path standing at exactly a
    /// mapping's prefix already gets, and for the same reason: a mapping reaches
    /// the path and no file on *this* device can stand for it, so it is reported
    /// rather than sanitized into some other local name (spec: EP-2, EP-4).
    ///
    /// A root that will not vouch for itself becomes
    /// [`RefusedRoot`](Self::RefusedRoot), which is the verdict a *single* write
    /// gets. A folder fetch takes that refusal out of the descent before it
    /// reaches here, because it has a mapping to report and other mappings to
    /// go on with (spec: EP-11, EP-13).
    ///
    /// `prefix` is the mapping the descent was made through, which the
    /// capability's own refusal cannot carry: [`Destinations::reach`] is handed
    /// the root and the components apart and knows nothing of Entry Paths, so
    /// the mapping is named back on this side of that call.
    ///
    /// Every call site hands one over all the same, and none of them can reach
    /// the arm it feeds: `reach` is the one operation that holds a root against
    /// the identity its mapping recorded, the one call here that makes a descent
    /// takes `Refused` out of the answer before it gets this far, and every
    /// other site is an operation against a folder `reach` has already vouched
    /// for. The argument is what the variant asks for rather than a case any of
    /// these sites is known to produce.
    ///
    /// [`Destinations::reach`]: crate::Destinations::reach
    pub(super) fn from_descent(
        refused: DescentError,
        prefix: Option<&EntryPath>,
        path: &EntryPath,
    ) -> Self {
        match refused {
            DescentError::Blocked { stopped_at } => Self::UnmaterializablePath {
                path: path.clone(),
                stopped_at: Some(stopped_at),
            },
            DescentError::Refused { root, reason } => Self::RefusedRoot {
                prefix: prefix.cloned(),
                local_root: root,
                reason,
            },
            DescentError::Io(refused) => Self::Io {
                operation: refused.operation,
                path: refused.path,
                cause: refused.cause,
            },
        }
    }
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
            // message carries it — which is why a diagnostic event renders
            // them through [`Redacted`] instead: an Entry Path never belongs
            // in one. Where a descent is what refused, the folder it stopped
            // at is in the value and the message names it: that one folder is
            // what a person can go and look at, and sending them to the
            // mappings instead would be sending them to the one thing that is
            // in order.
            Self::UnmaterializablePath {
                path,
                stopped_at: Some(stopped_at),
            } => write!(
                f,
                "the Entry Path {:?} cannot be materialized on this device: {}, on the way to \
                 it under the mapped root, is a symbolic link or an ordinary file rather than \
                 a folder",
                path.as_str(),
                stopped_at.display(),
            ),
            // Nothing on disk was reached, so the path itself is the whole of
            // the answer, and there is one way left for it to be: the path names
            // the folder a mapping is rooted at rather than anything inside it.
            // A component coffret keeps for itself is the variant below, which
            // says which component it was.
            Self::UnmaterializablePath {
                path,
                stopped_at: None,
            } => write!(
                f,
                "the Entry Path {:?} cannot be materialized on this device: it names exactly a \
                 mapped root, which is the folder the subtree lives in and cannot also be a file \
                 in it",
                path.as_str()
            ),
            // The component is named because it is the whole of what a person
            // changes: the path is theirs to spell and exactly one name in it is
            // not available.
            Self::ReservedComponent { path, component } => write!(
                f,
                "the Entry Path {:?} carries {component:?}, which coffret keeps for itself inside \
                 a mapped folder: a file there is one a scan steps over and no sync carries in, \
                 so nothing was placed",
                path.as_str(),
            ),
            // The folder is named, because it is the one thing there is to look
            // at, and the mapping is named beside it, because a device with
            // more than one leaves a person holding a gesture with nothing to
            // point it at. The prefix may be said here for the reason it may be
            // said to a browser: it is a name inside the Library rather than a
            // path (spec: EL-1). The gesture comes with both: what gets a root
            // out of any of these states is recording that mapping again
            // (spec: EP-13).
            Self::RefusedRoot {
                prefix,
                local_root,
                reason,
            } => write!(
                f,
                "{} is not the folder {} was recorded against: {reason}; nothing was placed into \
                 it, and recording that mapping again is what settles which folder it is",
                local_root.display(),
                mapping_named(prefix.as_ref()),
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
            // An Entry Path is what identifies each of these two, so the
            // message carries it, and [`Redacted`] is what a diagnostic event
            // gets instead.
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
            // The marker's own refusal where that is what made it, so a chain
            // printed from here ends at what the file held rather than at the
            // root.
            Self::RefusedRoot { reason, .. } => match reason {
                RootRefused::MarkerMalformed { cause } => Some(cause),
                _ => None,
            },
            Self::UnmaterializablePath { .. }
            | Self::ReservedComponent { .. }
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

impl Redacted for FetchError {
    /// Which refusal it is, the opaque identifiers behind it, and how long the
    /// path was.
    ///
    /// This is the vocabulary the rule exists for. Seven of its variants are
    /// *identified* by an Entry Path — that is what makes them the answer they
    /// are, and it is why the message names one — so the message is exactly
    /// what a diagnostic event must not render. What goes in instead is the
    /// variant and the path's length, which tells a reader whether a run met
    /// the same Entry over and over or a different one each time without
    /// saying which.
    ///
    /// The Container IDs stay: they are values this Library minted for objects
    /// whose names say nothing about their contents, and they are what makes
    /// an integrity verdict something a person can go and look into.
    fn redacted(&self) -> String {
        match self {
            Self::Storage(error) => format!("Fetch::Storage: {}", error.redacted()),
            Self::Index(error) => format!("Fetch::Index: {}", error.redacted()),
            Self::Format(error) => format!("Fetch::Format: {}", error.redacted()),
            Self::Commit(error) => format!("Fetch::Commit: {}", error.redacted()),
            Self::Io {
                operation, cause, ..
            } => format!("Fetch::Io(operation={operation}, kind={:?})", cause.kind()),
            // Which of the two ways it could not be materialized, since they
            // send a person to different places: a descent that stopped at a
            // folder on this device, or a path no local name can be made of at
            // all. The folder itself is a local path and stays out.
            Self::UnmaterializablePath { path, stopped_at } => format!(
                "Fetch::UnmaterializablePath(path_len={}, descent={})",
                path.as_str().len(),
                match stopped_at {
                    Some(_) => "blocked",
                    None => "unspellable",
                },
            ),
            // The component is a piece of the Entry Path, so it stays out for
            // the reason the path does: what is left is the length, which says
            // whether a run met the same path over and over.
            Self::ReservedComponent { path, .. } => {
                format!("Fetch::ReservedComponent(path_len={})", path.as_str().len())
            }
            // Which shape the wrong folder took, which is the whole of what an
            // event is for here: the folder itself is a local path and stays
            // out, and so does either identity.
            Self::RefusedRoot { reason, .. } => {
                format!("Fetch::RefusedRoot: {}", reason.redacted())
            }
            Self::LocalPathCollision { first, second } => format!(
                "Fetch::LocalPathCollision(first_len={}, second_len={})",
                first.as_str().len(),
                second.as_str().len(),
            ),
            Self::ContainerUnreachable { container_id } => {
                format!("Fetch::ContainerUnreachable(container={container_id})")
            }
            Self::CiphertextMismatch {
                container_id,
                expected,
                actual,
            } => format!(
                "Fetch::CiphertextMismatch(container={container_id}, expected={}, actual={})",
                hex(expected),
                hex(actual),
            ),
            Self::EntryMissing { container_id, path } => format!(
                "Fetch::EntryMissing(container={container_id}, path_len={})",
                path.as_str().len(),
            ),
            Self::ContentMismatch { container_id, path } => format!(
                "Fetch::ContentMismatch(container={container_id}, path_len={})",
                path.as_str().len(),
            ),
            Self::UnmappedContainer { container_id } => {
                format!("Fetch::UnmappedContainer(container={container_id})")
            }
            Self::EntryNotCurrent { path } => {
                format!("Fetch::EntryNotCurrent(path_len={})", path.as_str().len())
            }
            Self::UnmappedEntryPath { path } => {
                format!("Fetch::UnmappedEntryPath(path_len={})", path.as_str().len())
            }
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

/// How a message names the mapping a refusal is about (spec: EP-13).
///
/// The Library-side prefix, or the Library root where the mapping stands for
/// that and there is no component to name. Never the local root: that is the
/// message's own to name, and it is named beside this rather than instead of it.
fn mapping_named(prefix: Option<&EntryPath>) -> String {
    match prefix {
        Some(prefix) => format!("the mapping for {:?}", prefix.as_str()),
        None => "the mapping for the Library root".to_owned(),
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::entry_paths::entry_path;

    fn path() -> EntryPath {
        entry_path("albums/spring.jpg")
    }

    // EL-1, EP-9: the message is written for whoever is keeping the Library and
    // names the path they asked about; the diagnostic event says which refusal
    // it was.
    #[test]
    fn a_path_no_mapping_reaches_is_named_to_a_person_and_not_to_the_log() {
        let error = FetchError::UnmappedEntryPath { path: path() };

        assert!(error.to_string().contains("albums/spring.jpg"));
        assert_eq!(error.redacted(), "Fetch::UnmappedEntryPath(path_len=17)");
    }

    // EP-4: the folder a descent stopped at is a local path, which is the other
    // half of what may never be written down — and which of the two shapes the
    // refusal took is kept, because they send a person to different places.
    #[test]
    fn a_blocked_descent_keeps_its_shape_and_loses_both_paths() {
        let blocked = FetchError::UnmaterializablePath {
            path: path(),
            stopped_at: Some(PathBuf::from("/home/someone/albums")),
        };
        let unspellable = FetchError::UnmaterializablePath {
            path: path(),
            stopped_at: None,
        };

        assert!(blocked.to_string().contains("/home/someone/albums"));
        assert_eq!(
            blocked.redacted(),
            "Fetch::UnmaterializablePath(path_len=17, descent=blocked)",
        );
        assert_eq!(
            unspellable.redacted(),
            "Fetch::UnmaterializablePath(path_len=17, descent=unspellable)",
        );
    }

    // EP-14, EL-1: the component is what a person changes, so the message names
    // it — and it is a piece of their own path, so the event holds neither it
    // nor the path it came out of.
    #[test]
    fn a_reserved_component_is_named_to_a_person_and_not_to_the_log() {
        let error = FetchError::ReservedComponent {
            path: entry_path("albums/.coffret/root"),
            component: ".coffret".to_owned(),
        };

        let said = error.to_string();
        assert!(said.contains("albums/.coffret/root"), "{said}");
        assert!(said.contains(".coffret\""), "{said}");
        assert_eq!(error.redacted(), "Fetch::ReservedComponent(path_len=20)");
    }

    // An integrity verdict is worth reading, and reading one means knowing
    // which object it is about — which is a name this Library minted.
    #[test]
    fn an_integrity_verdict_keeps_the_container_it_is_about() {
        let error = FetchError::ContentMismatch {
            container_id: ContainerId::from_bytes([0x11; ContainerId::BYTE_LEN]),
            path: path(),
        };
        let redacted = error.redacted();

        assert!(
            redacted.starts_with("Fetch::ContentMismatch(container="),
            "{redacted}"
        );
        assert!(redacted.ends_with("path_len=17)"), "{redacted}");
        assert!(!redacted.contains("albums"), "{redacted}");
    }
}
