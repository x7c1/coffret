use std::error;
use std::fmt;

use coffret_model::{ContainerId, ControlObjectKind, ControlObjectName, EntryPath, Generation};

use crate::error::Error;
use crate::index_error::IndexError;

/// Result alias for the commit flow.
pub type CommitResult<T> = std::result::Result<T, CommitError>;

/// Everything a commit can fail with.
///
/// It is a vocabulary of its own rather than more variants on
/// [`Error`], and the reason is what that type is documented to
/// be: the storage vocabulary a gateway translates its provider's answers into.
/// A commit fails at things no provider ever reports — a batch whose Entry
/// Paths collide, a Keyring candidate that did not come back complete, a rebase
/// that ran out of attempts — and putting those beside `RateLimited` would make
/// [`Error::is_retryable`](crate::Error::is_retryable) answer questions it was
/// never asked. So every verdict the flow itself reaches gets a variant here —
/// those three among them, alongside what it decides about the control state it
/// read — and what the ports and the format layer report travels inside
/// [`CommitError::Storage`], [`CommitError::Index`], and
/// [`CommitError::Format`] unchanged.
///
/// There is deliberately no `PartialEq`, here or on the two values its variants
/// carry ([`InvalidReplica`], [`ControlObjectFault`]): a caller decides from the
/// variant and the fields it names, never by comparing two errors.
#[derive(Debug)]
pub enum CommitError {
    /// Storage failed, or answered something the flow cannot commit on.
    Storage(Error),
    /// The Index could not be brought to, or moved past, a committed state.
    Index(IndexError),
    /// A control object could not be encoded, or what Storage held could not be
    /// opened as one.
    Format(coffret_format::Error),
    /// The batch would put two current Entries at one Entry Path.
    ///
    /// Removals leave the current path map first and additions enter after, so
    /// a path may move from a replaced Container to its replacement within one
    /// batch; what is refused is a path two surviving Entries would both claim
    /// (spec: EP-5, EP-6). The refusal happens before anything is written, and
    /// a writer that lost the commit race repeats it against the new head
    /// rather than resolving it by timestamp (spec: CP-7, EP-7).
    EntryPathCollision {
        /// The path claimed twice.
        path: EntryPath,
    },
    /// A Container that survives the batch has no entry in the committed
    /// Keyring.
    ///
    /// At every commit boundary the committed Keyring maps every current
    /// Container, to an envelope or to an explicit key-lost marker (spec:
    /// KL-7). One that maps to neither leaves the next generation with nothing
    /// to carry over, and inventing a marker would record a key loss the
    /// Library never suffered.
    UnmappedContainer {
        /// The Container the committed Keyring says nothing about.
        container_id: ContainerId,
    },
    /// No replica of the committed Keyring read back valid.
    ///
    /// One valid replica holds the whole mapping (spec: KL-6), so a generation
    /// that answers with none leaves the next one nothing to carry over.
    /// Whether that is the Keyring loss RV-7 names — zero committed valid
    /// replicas, which repair cannot help — or a Storage failure a later run
    /// gets past is what `cause` carries: the walk stops either way, and
    /// deciding between the two is not this flow's to make.
    KeyringUnreadable {
        /// The generation none of whose replicas answered.
        generation: Generation,
        /// Which replica the walk tried last.
        replica: u16,
        /// What that replica was refused for.
        cause: InvalidReplica,
    },
    /// A replica of the candidate Keyring was missing or invalid on read-back.
    ///
    /// The candidate set must be complete before the commit selects it (spec:
    /// CP-8, KL-2), so the flow stops here with the Journal untouched. The
    /// replicas already written stay where they are: they are an uncommitted
    /// candidate, which selects nothing (spec: KL-3).
    IncompleteKeyring {
        /// The generation the candidate belongs to.
        generation: Generation,
        /// Which replica index did not come back valid.
        replica: u16,
        /// What reading it back found instead.
        cause: InvalidReplica,
    },
    /// The commit was rebased as often as the policy allows and still lost.
    ///
    /// Not a conflict that needs resolving — every attempt rebased cleanly
    /// (spec: CP-4) — but a Library busy enough that this device never got the
    /// slot. A later run starts again from the head it reached.
    ConflictLimitReached {
        /// How many attempts were made.
        attempts: u32,
    },
    /// A head the replay has to read is not on Storage.
    ///
    /// Catching up replays the Journal after its starting point, so a gap in
    /// the chain is a Library that cannot be caught up with rather than a
    /// commit that can proceed without it (spec: CK-9).
    MissingHead {
        /// The generation whose head object is gone.
        generation: Generation,
    },
    /// The head chain carries a Master Key epoch activation.
    ///
    /// A writer whose slot was consumed by an activation Snapshot stops until it
    /// is re-enrolled in the new epoch (spec: CP-5), and a device replaying past
    /// one is in the same position: what follows is sealed under a Master Key it
    /// does not have.
    EpochActivated {
        /// The head generation the activation took.
        generation: Generation,
    },
    /// An object is not the control object its name and position promised.
    ///
    /// Reported rather than overwritten or written under another name: a second
    /// name for one head would leave readers two checkpoints to choose between
    /// (spec: CK-11).
    CorruptControlObject {
        /// The name the object was read under, as the value it stands for
        /// rather than as text, so that a caller can ask which position was
        /// claimed without parsing the name again.
        object: ControlObjectName,
        /// What about it did not hold.
        fault: ControlObjectFault,
    },
}

/// Why one replica of a Keyring generation is not one a mapping may be read
/// from (spec: KL-1).
///
/// A replica is read back the same way wherever a Keyring is read, and what
/// differs is what the reader does with a failure: a committed set steps over
/// the replica and tries the next, because one valid replica carries the whole
/// mapping (spec: KL-6), while a candidate set stops the commit, because a set
/// that is not complete is not one a commit may select (spec: CP-8, KL-2). So
/// the reason travels as a value and the reader wraps it in whichever of
/// [`CommitError::KeyringUnreadable`] and [`CommitError::IncompleteKeyring`]
/// says which of the two decisions it made.
#[derive(Debug)]
pub enum InvalidReplica {
    /// The commitment declares the replica and Storage does not hold it.
    Absent,
    /// Fetching it failed, or what came back could not be opened.
    ///
    /// Whatever Storage or the format layer reported, in this flow's own
    /// vocabulary rather than in two spellings a caller would have to match
    /// separately.
    Unreadable(Box<CommitError>),
    /// It opened as another kind of control object.
    KindNotAdmitted {
        /// The kind its authenticated header declares.
        found: ControlObjectKind,
    },
    /// The mapping it holds is not the one its name promises (spec: CP-10,
    /// KL-14).
    DigestMismatch {
        /// The digest the replica's name carries.
        expected: String,
        /// The digest of the mapping the object holds.
        actual: String,
    },
}

/// What about a control object did not hold (spec: FM-11, FM-12).
///
/// It is what [`CommitError::CorruptControlObject`] carries alongside the name
/// the object was read under, so the reason is a value a caller can act on and
/// not a sentence it would have to read.
#[derive(Debug)]
pub enum ControlObjectFault {
    /// It is not a control object this build can open.
    Unopenable(coffret_format::Error),
    /// Its header declares a kind the position it was read at does not admit.
    KindNotAdmitted {
        /// The kind its authenticated header declares.
        found: ControlObjectKind,
    },
    /// Its authenticated header stands at another generation than its name
    /// carries.
    GenerationMismatch {
        /// The generation the header states.
        found: Generation,
    },
    /// It checkpoints a head other than the one whose snapshot slot holds it.
    CheckpointsAnotherHead {
        /// The head the Snapshot inside stands at.
        found: Generation,
    },
}

impl fmt::Display for CommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(f, "{error}"),
            Self::Index(error) => write!(f, "{error}"),
            Self::Format(error) => write!(f, "{error}"),
            // The Entry Path is what identifies the conflict, so the message
            // carries it — and the flow therefore logs nothing about it,
            // because an Entry Path never belongs in a log line.
            Self::EntryPathCollision { path } => {
                // Quoted, and quoted around the path itself: a path is the one
                // field here that can carry spaces, and `{path:?}` would spell
                // the wrapper type's name around it rather than quote it.
                write!(
                    f,
                    "two current Entries would claim the Entry Path {:?}",
                    path.as_str()
                )
            }
            Self::UnmappedContainer { container_id } => write!(
                f,
                "the committed Keyring holds neither an envelope nor a key-lost \
                 marker for Container {container_id}"
            ),
            Self::KeyringUnreadable {
                generation,
                replica,
                cause,
            } => write!(
                f,
                "no valid replica of Keyring generation {generation} could be read; \
                 replica {replica} was the last tried, and {cause}"
            ),
            Self::IncompleteKeyring {
                generation,
                replica,
                cause,
            } => write!(
                f,
                "replica {replica} of the candidate Keyring generation {generation} \
                 did not read back valid: {cause}"
            ),
            Self::ConflictLimitReached { attempts } => write!(
                f,
                "the commit slot was taken by another writer on all {attempts} attempts"
            ),
            Self::MissingHead { generation } => {
                write!(f, "the head at generation {generation} is not in Storage")
            }
            Self::EpochActivated { generation } => write!(
                f,
                "a Master Key epoch was activated at generation {generation}; \
                 this device must be re-enrolled before it can commit"
            ),
            Self::CorruptControlObject { object, fault } => {
                write!(f, "{object} is not the control object it promised: {fault}")
            }
        }
    }
}

impl error::Error for CommitError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            Self::Index(error) => Some(error),
            Self::Format(error) => Some(error),
            Self::KeyringUnreadable { cause, .. } | Self::IncompleteKeyring { cause, .. } => {
                Some(cause)
            }
            Self::CorruptControlObject { fault, .. } => Some(fault),
            _ => None,
        }
    }
}

impl fmt::Display for InvalidReplica {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absent => f.write_str("it is not in Storage"),
            Self::Unreadable(error) => write!(f, "it could not be read: {error}"),
            Self::KindNotAdmitted { found } => write!(f, "it carries a {found:?}, not a Keyring"),
            Self::DigestMismatch { expected, actual } => write!(
                f,
                "its mapping digests to {actual}, and its name promises {expected}"
            ),
        }
    }
}

impl error::Error for InvalidReplica {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Unreadable(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

impl fmt::Display for ControlObjectFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unopenable(error) => write!(f, "the format layer refused it: {error}"),
            Self::KindNotAdmitted { found } => write!(
                f,
                "it carries a {found:?}, which the position it was read at does not admit"
            ),
            Self::GenerationMismatch { found } => {
                write!(f, "its header stands at generation {found}")
            }
            Self::CheckpointsAnotherHead { found } => {
                write!(f, "it checkpoints the head at generation {found}")
            }
        }
    }
}

impl error::Error for ControlObjectFault {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Unopenable(error) => Some(error),
            _ => None,
        }
    }
}

impl From<Error> for CommitError {
    fn from(error: Error) -> Self {
        Self::Storage(error)
    }
}

impl From<IndexError> for CommitError {
    fn from(error: IndexError) -> Self {
        Self::Index(error)
    }
}

impl From<coffret_format::Error> for CommitError {
    fn from(error: coffret_format::Error) -> Self {
        Self::Format(error)
    }
}

impl From<coffret_model::Error> for CommitError {
    /// A value the domain does not admit reached the flow through Storage's own
    /// vocabulary, where [`Error::Model`] already stands
    /// for it — the last representable generation having no successor, say.
    /// Keeping one spelling means a caller matches one variant rather than two.
    fn from(error: coffret_model::Error) -> Self {
        Self::Storage(Error::Model(error))
    }
}
