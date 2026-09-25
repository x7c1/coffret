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
/// be: the storage vocabulary a gateway classifies its provider's answers into.
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
/// There is deliberately no `PartialEq`, here or on the three values its
/// variants carry ([`InvalidReplica`], [`UnrepairedReplica`],
/// [`ControlObjectFault`]): a caller decides from the variant and the fields it
/// names, never by comparing two errors.
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
    /// The committed Keyring is degraded and the repair did not complete.
    ///
    /// A committed set that has lost replicas must be complete again before
    /// another write (spec: KL-11), so a repair that could not restore every
    /// declared position leaves the gate closed: this commit writes nothing and
    /// commits nothing, while reads go on from the replicas that survive
    /// (spec: KL-16). The gate is never partially relaxed — a set one replica
    /// short refuses the commit exactly as a set three short does — and the
    /// next run examines the set again and tries the repair afresh.
    UnrepairedKeyring {
        /// The committed generation whose set is degraded (spec: KL-5).
        generation: Generation,
        /// Every position still short of a valid replica, ascending.
        ///
        /// The positions and not merely how many, for the reason
        /// [`KeyringRepair::rewritten`](super::KeyringRepair::rewritten) keeps
        /// them: a caller comparing two runs and finding the same position
        /// short each time is reading about one object rather than about a
        /// number. Positions the repair did rewrite are not among them; the one
        /// `replica` names is.
        needed: Vec<u16>,
        /// Every position the same examination did rewrite, ascending.
        ///
        /// A repair that stopped is still a repair performed, and what a device
        /// performed is what KL-15 obliges it to surface. These positions were
        /// missing or unreadable, were rewritten from a committed valid replica,
        /// and read back valid (spec: KL-6, KL-13, KL-14), so the set the next
        /// run meets is this much less short than the one this run met.
        ///
        /// They travel on the refusal because a refused commit produces no
        /// [`CommitOutcome`](super::CommitOutcome) to carry them: this value is
        /// the whole of what the run hands back, and work recorded only in a
        /// diagnostic event never reaches the person who asked for the run
        /// (spec: EL-1). Disjoint from `needed` — every declared position was
        /// found valid, is in one, or is in the other.
        rewritten: Vec<u16>,
        /// The position the repair stopped being able to complete at.
        ///
        /// The first of `needed` in the walk's order, and the one `cause` is
        /// about. Every position in need is attempted, so the rest of `needed`
        /// failed too and this is the reason to start from.
        replica: u16,
        /// What stopped it there.
        cause: UnrepairedReplica,
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
/// differs is what the reader does with a failure. A read of a committed set
/// steps over the replica and tries the next, because one valid replica carries
/// the whole mapping (spec: KL-6); a candidate set stops the commit, because a
/// set that is not complete is not one a commit may select (spec: CP-8, KL-2);
/// and a commit examining the committed set before it writes rewrites the
/// position instead, because that set is one it owes a repair (spec: KL-11,
/// KL-13). So the reason travels as a value and the reader wraps it in whichever
/// of [`CommitError::KeyringUnreadable`], [`CommitError::IncompleteKeyring`],
/// and [`UnrepairedReplica::Unconfirmed`] says which of those it made.
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

/// Why one position of a degraded committed Keyring is still not one a valid
/// replica stands at (spec: KL-13, KL-16).
///
/// [`InvalidReplica`] says why a replica could not be *read*; this says why the
/// repair that answer called for did not finish. The two are kept apart because
/// a reader steps over a bad replica and a repair is obliged to replace it, so
/// the vocabulary a repair reports in has a state the reader's has not: a
/// position left deliberately alone.
///
/// That state is [`Unfetchable`](Self::Unfetchable), and it is the reason this
/// is three variants rather than one. A replica Storage would not hand over is
/// not known to be lost — the object at that name may be exactly what it
/// promises — so KL-13's "rewrites the missing replicas" does not reach it and
/// the repair writes nothing there. The set cannot be called complete on that
/// evidence either, so the position still holds the gate closed (spec: KL-16).
/// The other two are ordinary failures of the write KL-14 defines: Storage
/// refused it, or it was acknowledged and the read-back that confirms it found
/// something that is still not a valid replica.
#[derive(Debug)]
pub enum UnrepairedReplica {
    /// Storage would not hand over the object this position holds.
    ///
    /// Nothing about its content is known, so it was not rewritten. What
    /// Storage reported travels inside, in this flow's own vocabulary.
    ///
    /// It carries [`InvalidReplica::Unfetchable`]'s name because it is that
    /// same finding: the read of the position answered nothing about the
    /// object, and here that answer is also the verdict on the repair.
    Unfetchable(Box<CommitError>),
    /// Storage refused the rewrite.
    ///
    /// Quota, permissions, or a provider having a bad minute — the retry policy
    /// has already spent what it was willing to (spec: KL-16). What Storage
    /// reported travels inside.
    Unwritten(Box<CommitError>),
    /// The rewrite was acknowledged and the replica did not read back valid.
    ///
    /// A repair confirms itself by reading the replica back, and what that
    /// read-back establishes is the replica's validity (spec: KL-14). This is
    /// that read-back's own verdict.
    Unconfirmed(InvalidReplica),
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
            // These three say which layer the commit was in and nothing more,
            // because `source` hands that layer's own error on and a caller
            // walking the chain prints both. Rendering the cause here as well
            // would spell one refusal twice over.
            Self::Storage(_) => f.write_str("the commit did not get what it asked of Storage"),
            Self::Index(_) => f.write_str("the commit could not read or write the Index"),
            Self::Format(_) => f.write_str("the commit could not encode or open a control object"),
            // The Entry Path is what identifies the conflict, so the message
            // carries it — which is why a diagnostic event renders this
            // through [`Redacted`] instead: an Entry Path never belongs in one.
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
            // The generations and the replica positions are this layer's
            // bookkeeping and the whole of what it knows the layer below does
            // not. Which rule the value missed, and what was wrong with the
            // replica, are the causes' own answers and `source` hands them on,
            // so a caller printing the chain reads each part once.
            Self::UnwritableControlValue { .. } => {
                f.write_str("this commit assembled a control value the rules do not admit")
            }
            Self::KeyringUnreadable {
                generation,
                replica,
                ..
            } => write!(
                f,
                "no valid replica of Keyring generation {generation} could be read; \
                 replica {replica} was the last tried"
            ),
            Self::IncompleteKeyring {
                generation,
                replica,
                ..
            } => write!(
                f,
                "replica {replica} of the candidate Keyring generation {generation} \
                 did not read back valid"
            ),
            // The four things a person can act on, in the order they need
            // them: what is wrong with the Library, that nothing of the batch
            // was committed, that their files are still readable all the same,
            // and that running again is the whole of the gesture — with what
            // the run did put back in between, so that a refused run is not
            // read as a wasted one (spec: KL-15, KL-16). Reads are on the
            // sentence because a backup tool refusing to write is a tool whose
            // user's first question is whether the copies already there still
            // come back, and the gate KL-16 closes is the write one only. The
            // positions are counted rather than listed, because which of them
            // it was decides nothing a person does; the reason the repair
            // stopped does, and that is the one thing spelled out.
            //
            // "Short of a valid replica at" and not "short of its replicas",
            // because what stopped the repair may be
            // [`UnrepairedReplica::Unfetchable`] — a position whose object is
            // not known to be gone at all. What every one of the three has in
            // common is that no valid replica stands there, which is what
            // `needed` is documented to hold, and a person told a replica was
            // lost and then told in the same breath that Storage merely would
            // not hand it over is reading two claims. Which of the three it
            // was is the cause's own sentence and `source` hands it on, so it
            // is read under this line rather than inside it as well.
            Self::UnrepairedKeyring {
                generation,
                needed,
                rewritten,
                replica,
                ..
            } => write!(
                f,
                "the committed Keyring generation {generation} is short of a valid replica at \
                 {} of its positions and could not be repaired: the repair stopped at replica \
                 {replica}{}; nothing of this batch was committed, reads and restores go on \
                 from the replicas that survive, and running again examines the set and repairs \
                 it afresh",
                needed.len(),
                rewritten_clause(rewritten),
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
            // Which object it was is what this layer knows; what is wrong with
            // it is the fault's own answer, which `source` hands on.
            Self::CorruptControlObject { object, .. } => {
                write!(f, "{object} is not the control object it promised")
            }
        }
    }
}

/// What the same examination put back, as a clause of the refusal, or nothing
/// at all where it put back no position.
///
/// A person told only what is still short would read a refused run as a wasted
/// one, and it was not: the positions it rewrote stand, and the set the next run
/// meets is that much less short (spec: KL-15). Silent where there is nothing to
/// report, because a sentence that says "0 replicas were rewritten" spends a
/// clause of a refusal on news that never arrived.
fn rewritten_clause(rewritten: &[u16]) -> String {
    match rewritten.len() {
        0 => String::new(),
        1 => "; 1 other replica was rewritten and stands".to_owned(),
        many => format!("; {many} other replicas were rewritten and stand"),
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
            Self::UnrepairedKeyring { cause, .. } => Some(cause),
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
            // Which way the replica was no good, and not what Storage or the
            // format layer answered: `source` hands that value on, and a
            // caller printing the chain would otherwise read it twice.
            Self::Unfetchable(_) => f.write_str("Storage would not hand it over"),
            Self::Unreadable(_) => f.write_str("it could not be opened"),
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

impl fmt::Display for UnrepairedReplica {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // "was not rewritten" first, because a person reading this beside
            // the two below has to be told that this one position was left
            // exactly as it stands rather than written at and refused. What
            // Storage or the reading answered is the value `source` hands on
            // and is read under this line, never inside it as well.
            //
            // The position is named rather than left to a pronoun, because of
            // where this line lands: the wrapper that carries it says which
            // replica the repair stopped at and then spends a clause and a
            // half on what the run did and what to do next, so by the time a
            // chain reaches this the nearest thing an "it" could point at is
            // the batch or the set.
            Self::Unfetchable(_) => f.write_str(
                "that replica was not rewritten, because Storage would not hand over what it \
                 holds",
            ),
            Self::Unwritten(_) => f.write_str("that replica could not be written"),
            Self::Unconfirmed(_) => {
                f.write_str("that replica was written and did not read back valid")
            }
        }
    }
}

impl error::Error for UnrepairedReplica {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Unfetchable(error) | Self::Unwritten(error) => Some(error.as_ref()),
            Self::Unconfirmed(cause) => Some(cause),
        }
    }
}

impl Redacted for UnrepairedReplica {
    /// Which way the position was left short, with whatever refused it
    /// underneath.
    fn redacted(&self) -> String {
        match self {
            Self::Unfetchable(error) => format!("Repair::Unfetchable: {}", error.redacted()),
            Self::Unwritten(error) => format!("Repair::Unwritten: {}", error.redacted()),
            Self::Unconfirmed(cause) => format!("Repair::Unconfirmed: {}", cause.redacted()),
        }
    }
}

impl fmt::Display for ControlObjectFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // What the format layer refused it for is that layer's own answer,
            // which `source` hands on.
            Self::Unopenable(_) => f.write_str("the format layer refused it"),
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
            // How many positions are short, how many the run put back, and
            // which one stopped the repair, which is a commit's own bookkeeping
            // the whole way down: replica positions and a generation, and
            // nothing anybody chose (spec: EL-1, EL-3).
            Self::UnrepairedKeyring {
                generation,
                needed,
                rewritten,
                replica,
                cause,
            } => format!(
                "Commit::UnrepairedKeyring(generation={generation}, needed={}, \
                 rewritten={}, replica={replica}): {}",
                needed.len(),
                rewritten.len(),
                cause.redacted(),
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
            source: None,
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
    // the one thing in this vocabulary a diagnostic event may not carry.
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

    // KL-16: the refusal is one a person acts on, so the sentence has to carry
    // all three of what is wrong, what it cost them, and what ends it. The
    // diagnostic event carries the same finding as facts, and the chain reaches
    // what Storage said.
    //
    // KL-15: and where the same examination did put positions back before it
    // stopped, the sentence says so, because that work is a repair performed and
    // this refusal is the only thing a run that was refused hands back. A run
    // that put none back says nothing about it rather than reporting a zero.
    #[test]
    fn a_keyring_that_could_not_be_repaired_says_what_to_do_about_it() {
        let error = CommitError::UnrepairedKeyring {
            generation: Generation::FIRST,
            needed: vec![1, 2],
            rewritten: Vec::new(),
            replica: 1,
            cause: UnrepairedReplica::Unwritten(Box::new(provider_fault())),
        };

        let said = error.to_string();
        assert!(
            said.contains("short of a valid replica at 2 of its positions"),
            "{said}"
        );
        assert!(
            said.contains("nothing of this batch was committed"),
            "{said}"
        );
        assert!(
            said.contains("reads and restores go on"),
            "a write the gate refuses leaves reading alone, and the person told \
             their backup will not take a write asks about reading next \
             (spec: KL-16): {said}",
        );
        assert!(said.contains("running again"), "{said}");
        assert!(
            !said.contains("other replica"),
            "a run that put nothing back reports no repair rather than a zero: {said}",
        );
        assert!(
            error.source().is_some(),
            "the reason the repair stopped is the refusal's source",
        );
        assert_eq!(
            error.redacted(),
            format!(
                "Commit::UnrepairedKeyring(generation=0, needed=2, rewritten=0, replica=1): \
                 Repair::Unwritten: {}",
                provider_fault().redacted(),
            ),
        );

        let partial = CommitError::UnrepairedKeyring {
            generation: Generation::FIRST,
            needed: vec![2],
            rewritten: vec![0, 1],
            replica: 2,
            cause: UnrepairedReplica::Unwritten(Box::new(provider_fault())),
        };

        let said = partial.to_string();
        assert!(
            said.contains("short of a valid replica at 1 of its positions"),
            "{said}"
        );
        assert!(
            said.contains("2 other replicas were rewritten and stand"),
            "the repair it did perform is in the sentence: {said}",
        );
        assert_eq!(
            partial.redacted(),
            format!(
                "Commit::UnrepairedKeyring(generation=0, needed=1, rewritten=2, replica=2): \
                 Repair::Unwritten: {}",
                provider_fault().redacted(),
            ),
        );
    }

    // KL-13: a replica Storage would not hand over is not one a repair may
    // write over, and the three ways a position stays short have to be legible
    // as three rather than as one refusal worded differently.
    #[test]
    fn the_three_ways_a_repair_stops_do_not_read_alike() {
        let unfetchable = UnrepairedReplica::Unfetchable(Box::new(provider_fault()));
        let unwritten = UnrepairedReplica::Unwritten(Box::new(provider_fault()));
        let unconfirmed = UnrepairedReplica::Unconfirmed(InvalidReplica::Absent);

        assert!(
            unfetchable.to_string().contains("was not rewritten"),
            "a position left alone says so: {unfetchable}",
        );
        for (left, right) in [
            (&unfetchable, &unwritten),
            (&unwritten, &unconfirmed),
            (&unfetchable, &unconfirmed),
        ] {
            assert_ne!(left.to_string(), right.to_string());
            assert_ne!(left.redacted(), right.redacted());
        }
    }

    /// The links a caller printing `{error:#}` reads, outermost first.
    fn chain(error: &dyn error::Error) -> Vec<String> {
        let mut links = vec![error.to_string()];
        let mut below = error.source();
        while let Some(link) = below {
            links.push(link.to_string());
            below = link.source();
        }
        links
    }

    // A wrapper says which layer, the cause says what that layer answered, and
    // the chain a caller prints holds each of those once — all the way down
    // through the repair's own vocabulary to what Storage said.
    #[test]
    fn a_stopped_repair_reaches_a_caller_as_one_sentence_per_layer() {
        let error = CommitError::UnrepairedKeyring {
            generation: Generation::FIRST,
            needed: vec![1, 2],
            rewritten: Vec::new(),
            replica: 1,
            cause: UnrepairedReplica::Unwritten(Box::new(provider_fault())),
        };

        assert_eq!(
            chain(&error),
            vec![
                "the committed Keyring generation 0 is short of a valid replica at 2 of its \
                 positions and could not be repaired: the repair stopped at replica 1; nothing \
                 of this batch was committed, reads and restores go on from the replicas that \
                 survive, and running again examines the set and repairs it afresh"
                    .to_owned(),
                "that replica could not be written".to_owned(),
                "the commit did not get what it asked of Storage".to_owned(),
                "Storage failed with status 503: backendError".to_owned(),
            ],
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
