use std::error;
use std::fmt;

use coffret_model::{
    ContainerId, ControlObjectKind, ControlObjectName, EntryPath, Generation, Redacted,
};

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
    /// A control value this commit assembled is not one the rules admit.
    ///
    /// The Keyring mapping the next generation would carry, the entry table of
    /// a Container the batch spooled, the record the batch commits: each is
    /// built through the constructor that holds its own rules, so a refusal
    /// here is about what this device assembled rather than about anything
    /// Storage or another writer did — a batch that re-added a Container the
    /// held mapping still lists, say (spec: FM-17, KL-7). Nothing has been
    /// written when it is raised, and the next attempt starts from whatever the
    /// head turns out to be.
    UnwritableControlValue {
        /// What the domain refused, and why.
        cause: coffret_model::Error,
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
///
/// A fetch that failed and an object that arrived and was rejected are kept
/// apart, because they are different findings about the Library rather than two
/// spellings of one. An object that did arrive and could not be opened is a
/// replica that is definitively not one a mapping may be read from, so the set
/// it belongs to is a valid replica short: a committed set with one left is the
/// degraded state KL-5 names, with a repair owed to it (spec: KL-13), and a
/// candidate is one no commit may select (spec: KL-2). A caller
/// reading [`CommitError::KeyringUnreadable`] tells a Keyring that has lost
/// replicas from a provider that was merely having a bad minute by which of the
/// two it finds.
#[derive(Debug)]
pub enum InvalidReplica {
    /// The commitment declares the replica and Storage does not hold it.
    Absent,
    /// Storage did not hand the object over.
    ///
    /// Nothing about the replica's content is known: what failed is the fetch,
    /// and the object it was for may be exactly what its name promises. What
    /// Storage reported travels inside, in this flow's own vocabulary.
    Unfetchable(Box<CommitError>),
    /// The object arrived and could not be opened.
    ///
    /// Decrypting, authenticating, or decoding it failed, so this replica is
    /// definitively not one a mapping may be read from. What the format layer
    /// reported travels inside, in this flow's own vocabulary.
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
    /// The format layer would not hand a value back for it.
    ///
    /// Two findings under one name, and the refusal inside says which: bytes
    /// this build cannot open at all — a header it does not read, a tag that
    /// does not verify — and a payload that opened and was then refused for
    /// what it says. A Snapshot checkpointing a head other than the one its
    /// name is for is the second kind: the decoder is told the name's own
    /// generation and holds the rule there (spec: CK-10), so the refusal
    /// arrives here already saying which head was named and which was
    /// checkpointed.
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
}

impl fmt::Display for CommitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Storage(error) => write!(f, "{error}"),
            Self::Index(error) => write!(f, "{error}"),
            Self::Format(error) => write!(f, "{error}"),
            // The Entry Path is what identifies the conflict, so the message
            // carries it — which is why a log line renders this through
            // [`Redacted`] instead: an Entry Path never belongs in one.
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
            Self::UnwritableControlValue { cause } => write!(
                f,
                "this commit assembled a control value the rules do not admit: {cause}"
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
            Self::UnwritableControlValue { cause } => Some(cause),
            _ => None,
        }
    }
}

impl fmt::Display for InvalidReplica {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absent => f.write_str("it is not in Storage"),
            Self::Unfetchable(error) => write!(f, "Storage would not hand it over: {error}"),
            Self::Unreadable(error) => write!(f, "it could not be opened: {error}"),
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
            Self::Unfetchable(error) | Self::Unreadable(error) => Some(error.as_ref()),
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

impl Redacted for CommitError {
    /// The generations, the replicas, and the object names, and no Entry Path.
    ///
    /// Almost all of this vocabulary is already log-safe by what it is about:
    /// a commit's own bookkeeping is generations, replica positions, attempt
    /// counts and control-object names, none of which anybody chose. The one
    /// exception is [`EntryPathCollision`](Self::EntryPathCollision), which is
    /// *identified* by a path — and it is enough on its own to make rendering
    /// this type with `Display` unsafe.
    fn redacted(&self) -> String {
        match self {
            Self::Storage(error) => format!("Commit::Storage: {}", error.redacted()),
            Self::Index(error) => format!("Commit::Index: {}", error.redacted()),
            Self::Format(error) => format!("Commit::Format: {}", error.redacted()),
            Self::EntryPathCollision { path } => format!(
                "Commit::EntryPathCollision(path_len={})",
                path.as_str().len()
            ),
            Self::UnmappedContainer { container_id } => {
                format!("Commit::UnmappedContainer(container={container_id})")
            }
            Self::UnwritableControlValue { cause } => {
                format!("Commit::UnwritableControlValue: {}", cause.redacted())
            }
            Self::KeyringUnreadable {
                generation,
                replica,
                cause,
            } => format!(
                "Commit::KeyringUnreadable(generation={generation}, replica={replica}): {}",
                cause.redacted()
            ),
            Self::IncompleteKeyring {
                generation,
                replica,
                cause,
            } => format!(
                "Commit::IncompleteKeyring(generation={generation}, replica={replica}): {}",
                cause.redacted()
            ),
            Self::ConflictLimitReached { attempts } => {
                format!("Commit::ConflictLimitReached(attempts={attempts})")
            }
            Self::MissingHead { generation } => {
                format!("Commit::MissingHead(generation={generation})")
            }
            Self::EpochActivated { generation } => {
                format!("Commit::EpochActivated(generation={generation})")
            }
            Self::CorruptControlObject { object, fault } => format!(
                "Commit::CorruptControlObject(object={object}): {}",
                fault.redacted()
            ),
        }
    }
}

impl Redacted for InvalidReplica {
    /// Which way a replica was no good, with whatever refused it underneath.
    fn redacted(&self) -> String {
        match self {
            Self::Absent => "Replica::Absent".to_owned(),
            Self::Unfetchable(error) => {
                format!("Replica::Unfetchable: {}", error.redacted())
            }
            Self::Unreadable(error) => format!("Replica::Unreadable: {}", error.redacted()),
            Self::KindNotAdmitted { found } => {
                format!("Replica::KindNotAdmitted(found={found:?})")
            }
            Self::DigestMismatch { expected, actual } => {
                format!("Replica::DigestMismatch(expected={expected}, actual={actual})")
            }
        }
    }
}

impl Redacted for ControlObjectFault {
    /// Which way a control object was not the one it promised to be.
    fn redacted(&self) -> String {
        match self {
            Self::Unopenable(error) => format!("Control::Unopenable: {}", error.redacted()),
            Self::KindNotAdmitted { found } => {
                format!("Control::KindNotAdmitted(found={found:?})")
            }
            Self::GenerationMismatch { found } => {
                format!("Control::GenerationMismatch(found={found})")
            }
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

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use super::*;
    use crate::entry_paths::entry_path;

    /// What a provider that failed on its own side leaves behind.
    fn provider_fault() -> CommitError {
        CommitError::Storage(Error::ServiceUnavailable {
            status: 503,
            detail: "backendError".to_owned(),
        })
    }

    /// What an object that arrived and would not open leaves behind.
    fn unopenable() -> CommitError {
        CommitError::Format(coffret_format::Error::AuthenticationFailed)
    }

    #[test]
    fn a_fetch_that_failed_says_nothing_about_the_replica() {
        // The reading a caller has to be able to make: Storage was asked and
        // did not answer, so the Keyring's own health is unknown and repairing
        // the set is not what this reports.
        let cause = InvalidReplica::Unfetchable(Box::new(provider_fault()));
        let InvalidReplica::Unfetchable(inner) = &cause else {
            panic!("expected an unfetchable replica, got {cause:?}");
        };
        assert!(
            matches!(
                inner.as_ref(),
                CommitError::Storage(Error::ServiceUnavailable { .. })
            ),
            "what Storage reported travels inside, got {inner:?}",
        );
        assert!(
            cause.source().is_some(),
            "the fetch failure is the reason's source",
        );
    }

    #[test]
    fn an_object_that_arrived_and_would_not_open_is_a_bad_replica() {
        // The opposite reading: the object is on Storage and is not a replica
        // this Library can read, so the set it belongs to is a valid replica
        // short — which of the states KL-5 separates that leaves it in is the
        // reader's to say, and both readers construct this same value.
        let cause = InvalidReplica::Unreadable(Box::new(unopenable()));
        let InvalidReplica::Unreadable(inner) = &cause else {
            panic!("expected an unreadable replica, got {cause:?}");
        };
        assert!(
            matches!(inner.as_ref(), CommitError::Format(_)),
            "what the format layer reported travels inside, got {inner:?}",
        );
        assert!(
            cause.source().is_some(),
            "the refusal to open is the reason's source",
        );
    }

    // EP-6: the path is what identifies the conflict to a person, and it is
    // the one thing in this vocabulary a log line may not carry.
    #[test]
    fn two_entries_claiming_one_path_are_recorded_without_it() {
        let error = CommitError::EntryPathCollision {
            path: entry_path("albums/spring.jpg"),
        };

        assert!(error.to_string().contains("albums/spring.jpg"));
        assert_eq!(error.redacted(), "Commit::EntryPathCollision(path_len=17)");
    }

    // A commit's own bookkeeping is worth reading whole: nothing in it is
    // anybody's name for anything.
    #[test]
    fn a_keyring_that_would_not_read_keeps_its_generation_and_its_replica() {
        let error = CommitError::KeyringUnreadable {
            generation: Generation::FIRST,
            replica: 2,
            cause: InvalidReplica::Absent,
        };

        assert_eq!(
            error.redacted(),
            "Commit::KeyringUnreadable(generation=0, replica=2): Replica::Absent",
        );
    }

    #[test]
    fn the_two_findings_do_not_read_alike() {
        let unfetchable = InvalidReplica::Unfetchable(Box::new(provider_fault())).to_string();
        let unreadable = InvalidReplica::Unreadable(Box::new(unopenable())).to_string();
        assert_ne!(
            unfetchable, unreadable,
            "a person reading either one is told which of the two happened",
        );
    }
}
