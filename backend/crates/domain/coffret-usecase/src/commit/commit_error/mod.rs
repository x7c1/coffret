use std::error;
use std::fmt;

use coffret_model::{ContainerId, ControlObjectName, EntryPath, Generation, ObjectRef, Redacted};

use crate::commit::pending_change::PendingChange;
use crate::commit::rebase_rule::RebaseRule;
use crate::error::Error;
use crate::index_error::{IndexError, RenameRule};

// The three values the variants carry, each a module of its own beside the
// error that carries them.
mod control_object_fault;
pub use control_object_fault::ControlObjectFault;

mod unrepaired_replica;
pub use unrepaired_replica::UnrepairedReplica;

mod unusable_replica;
pub use unusable_replica::UnusableReplica;

#[cfg(test)]
mod fixtures;

#[cfg(test)]
mod tests;

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
/// variants carry ([`UnusableReplica`], [`UnrepairedReplica`],
/// [`ControlObjectFault`]): a caller decides from the variant and the fields it
/// names, never by comparing two errors.
#[derive(Debug, Clone)]
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
    /// The batch removes a Container that is no longer current.
    ///
    /// A batch's removals were decided against the head it was prepared on: the
    /// one-file Container a sync replaces, the ones a freeze absorbs, the Pack a
    /// deletion removes or rebuilds. When another writer has removed one of them
    /// since, committing the batch anyway would silently undo that removal — the
    /// replacement lands as if nothing happened, or a rebuilt Pack brings back
    /// Entries the other writer deleted — so the batch is refused as a conflict
    /// instead (spec: CP-7, CP-18). The refusal happens before anything is
    /// written, on the first attempt and on every rebase alike, and the batch is
    /// not offered again: the next run plans afresh from the state that removed
    /// these.
    RemovalNotCurrent {
        /// Every removal the current set no longer holds, ascending.
        container_ids: Vec<ContainerId>,
    },
    /// A record committed since the batch was prepared and one of the batch's
    /// pending changes write to one Entry or one Entry Path from two sides.
    ///
    /// A rebase examines every record after the head the batch was prepared
    /// against and applies their renames to the batch by `(Container ID, entry
    /// number)`: an update or a rebuilt Pack takes over the Entry at the path a
    /// rename gave it, and a rename follows the Entry an update replaced its
    /// Entry with. What it cannot carry over is this: a rename onto the path an
    /// addition wants, a rename of an Entry another writer renamed or replaced
    /// under another name, a rename onto a path another writer filled. Neither
    /// side is picked, so the batch is refused before anything is written and
    /// is not offered again; the next run plans from the Library as that record
    /// left it (spec: CP-4, CP-7, EP-7).
    RebaseConflict {
        /// The batch's change the rebase could not carry over.
        pending: PendingChange,
        /// The generation of the record it conflicts with.
        record: Generation,
        /// What that record did that conflicts with it.
        rule: RebaseRule,
        /// The Entry Path the two sides meet at: the path the addition wants,
        /// the name the rename was prepared against, or the path it gives.
        path: EntryPath,
    },
    /// A rename of the batch breaks one of the rules a record's renames are
    /// held to, against the head the batch would commit onto, and no record
    /// committed since the batch was prepared accounts for it (spec: FM-15).
    ///
    /// A record that broke one would be refused whole by every catalog that
    /// applied it, the committing device's own included, so it is never
    /// created; nothing has been written when this is raised. A rebase reports
    /// what another writer did as [`RebaseConflict`](Self::RebaseConflict)
    /// instead, so this is a batch its producer assembled against a catalog it
    /// misread.
    RenameRefused {
        /// The rename's position in the batch's renames.
        element: usize,
        /// Which rule it breaks.
        rule: RenameRule,
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
    /// One valid replica holds the whole key table (spec: KL-6), so a generation
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
        cause: UnusableReplica,
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
        /// Which replica position did not come back valid.
        replica: u16,
        /// What reading it back found instead.
        cause: UnusableReplica,
    },
    /// The committed Keyring set is short and the repair did not complete.
    ///
    /// A committed set must be shown complete before another write
    /// (spec: KL-2, KL-11), so a repair that could not restore every declared
    /// position — one it could not rewrite, or one Storage would not hand over
    /// — leaves the gate closed: this commit writes nothing and commits
    /// nothing, while reads and restores go on from the replicas that survive
    /// (spec: KL-16). The gate is never partially relaxed — a set one
    /// replica short refuses the commit exactly as a set three short does — and
    /// the next run examines the set again and tries the repair afresh.
    UnrepairedKeyring {
        /// The committed generation whose set the examination found short.
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
        /// [`CommitOutcome`](super::CommitOutcome) to carry them, and work
        /// recorded only in a diagnostic event never reaches the person who
        /// asked for the run (spec: EL-1). They are this examination's, about
        /// this `generation`, and kept apart from
        /// [`CommitFailure::repairs`](super::CommitFailure::repairs), which
        /// holds what earlier attempts of the same run completed. Disjoint from
        /// `needed` — every declared position was found valid, is in one, or is
        /// in the other.
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
    /// commit that can proceed without it (spec: CK-9). A `prune` on another
    /// device between this run's listing and its read leaves such a gap too;
    /// running again starts from the Snapshot that `prune` rested on
    /// (spec: CK-6).
    MissingHead {
        /// The generation whose head object is gone.
        generation: Generation,
    },
    /// The Library's first head, which this run wrote or found created into
    /// `head-0` by another writer first, is not in the listing read after it
    /// (spec: CP-2).
    ///
    /// Not a head that is gone, like [`MissingHead`](Self::MissingHead): it was
    /// written, and the listing has not caught up with it. Writing it again
    /// would put a second `head-0` beside the first on a Storage that mints
    /// identifiers — two control objects under one name, which every reader
    /// refuses (spec: FM-12) — so the run stops instead, and a later one finds
    /// the head once the listing shows it.
    FirstHeadUnlisted,
    /// Storage lists two control objects under one name.
    ///
    /// On a Storage that mints identifiers names are not unique, and a head or a
    /// checkpoint twice over says the exclusion its name stands for did not
    /// hold: two successors of one head, or two checkpoints of one
    /// (spec: CP-3, CK-11). Neither is the Library's more than the other, and
    /// whichever one a reader took the other would stay invisible to
    /// everything that reads by name, so the Library is inconsistent and
    /// nothing is read from it or written to it (spec: FM-12).
    DuplicateControlObject {
        /// The name both objects are listed under.
        object: ControlObjectName,
        /// The Storage's handle for the object listed first.
        ///
        /// Both handles are said, because the name alone does not tell the
        /// two apart and whoever puts the Library right has to: a handle is
        /// the provider's identifier and carries no secret.
        first: ObjectRef,
        /// The Storage's handle for the object listed second.
        second: ObjectRef,
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
    /// The key table the next Keyring generation would carry, the entry table of
    /// a Container the batch spooled, the record the batch commits: each is
    /// built through the constructor that holds its own rules, so a refusal
    /// here is about what this device assembled rather than about anything
    /// Storage or another writer did — a batch that re-added a Container the
    /// held key table still lists, say (spec: FM-17, KL-7). Nothing has been
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
            // The IDs are what identifies the conflict, and unlike a path they
            // are nobody's name for anything, so the sentence and the
            // diagnostic event carry the same list.
            //
            // "a commit" rather than "another writer": the commit that removed
            // them is usually another device's, but it can be this batch's own
            // earlier attempt whose create landed while its answer was lost,
            // and the sentence must not accuse somebody of that. The next step
            // is spoken here rather than through `advice`, because this
            // refusal has no source and so ends the chain already.
            Self::RemovalNotCurrent { container_ids } => write!(
                f,
                "the batch removes {} that a commit since it was prepared has already \
                 removed: {}; running again plans afresh from the Library as it stands now",
                match container_ids.len() {
                    1 => "a Container",
                    _ => "Containers",
                },
                listed(container_ids),
            ),
            // The path is what a person recognizes the conflict by, as with a
            // collision, so the sentence carries it and the diagnostic event
            // renders this through [`Redacted`]. The sentence ends the chain,
            // so the next step is spoken here, as a stale removal speaks it.
            Self::RebaseConflict {
                pending,
                record,
                rule,
                path,
            } => write!(
                f,
                "{pending} at the Entry Path {:?} conflicts with the commit at generation \
                 {record}, which {rule}; nothing of this batch was committed, and running \
                 again plans afresh from the Library as it stands now",
                path.as_str()
            ),
            Self::RenameRefused { element, rule } => {
                write!(f, "rename {element} of the batch is refused: {rule}")
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
            // The three things a person needs to know, in the order they need
            // them: what is wrong with the Library, that nothing of the batch
            // was committed, and that their files are still readable all the
            // same — with what the run did put back in between, so that a
            // refused run is not read as a wasted one (spec: KL-15, KL-16).
            // Reads are on the sentence because a backup tool refusing to
            // write is a tool whose user's first question is whether the copies
            // already there still come back, and the gate KL-16 closes is the
            // write one only. The positions are counted rather than listed,
            // because which of them it was decides nothing a person does; the
            // reason the repair stopped does, and that is the one thing spelled
            // out.
            //
            // What to do about it — run again — is not here: it is
            // [`advice`](CommitError::advice), which a shell prints after the
            // whole chain. A chain prints this line and then its `source`, so
            // advice spoken here would be read before the reason the repair
            // stopped, and a person is told what to do before they are told
            // why.
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
                 {replica}{}; nothing of this batch was committed, and reads and restores go on \
                 from the replicas that survive",
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
            Self::FirstHeadUnlisted => f.write_str(
                "the Library's first head was written but Storage does not list it yet; \
                 nothing of this batch was committed, and a later run finds the head once \
                 the listing shows it",
            ),
            Self::DuplicateControlObject {
                object,
                first,
                second,
            } => write!(
                f,
                "Storage holds two control objects named {object} ({first} and {second}), so \
                 the Library is inconsistent and neither of them was read"
            ),
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

/// Container IDs as one comma-separated list, in the order given.
fn listed(container_ids: &[ContainerId]) -> String {
    container_ids
        .iter()
        .map(ContainerId::to_string)
        .collect::<Vec<_>>()
        .join(", ")
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

impl CommitError {
    /// What a person can do about this refusal, where there is something to
    /// say beyond the refusal itself.
    ///
    /// Apart from `Display` because of where a person reads it. An error prints
    /// as a chain, each layer's sentence and then its `source`'s, so advice
    /// spoken in a sentence that has a source lands before the cause it is
    /// advice about. A shell prints this as its own line after the whole chain
    /// instead, and a diagnostic event never carries it: it is for the person
    /// at the terminal, and [`redacted`](Redacted::redacted) is unchanged by it.
    ///
    /// [`UnrepairedKeyring`](Self::UnrepairedKeyring) and a Journal record of a
    /// schema older than this build reads are the two refusals here that both
    /// advise and have a source. The others that say what to do —
    /// [`EpochActivated`](Self::EpochActivated) among them — have no source, so
    /// their sentence is the last line of the chain already and keeps its
    /// advice.
    pub fn advice(&self) -> Option<&'static str> {
        match self {
            // The whole of the gesture: the gate KL-16 closes is never
            // partially relaxed, and the next run examines the set and tries
            // the repair afresh.
            Self::UnrepairedKeyring { .. } => {
                Some("running again examines the committed Keyring and repairs it afresh")
            }
            // A Library written before the record schema this build reads is
            // not broken, and nothing here reads or repairs it: every record
            // of it is one this build refuses (spec: FM-15). The one gesture
            // is to make the Library again in the current format.
            Self::Format(coffret_format::Error::UnsupportedJournalRecordSchema { .. }) => Some(
                "this Library predates this build's format, and nothing of it is read or \
                 repaired — make it again with `coffret init` and bring its files in again",
            ),
            _ => None,
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
            Self::UnrepairedKeyring { cause, .. } => Some(cause),
            Self::CorruptControlObject { fault, .. } => Some(fault),
            Self::UnwritableControlValue { cause } => Some(cause),
            _ => None,
        }
    }
}

impl Redacted for CommitError {
    /// The generations, the replicas, and the object names, and no Entry Path.
    ///
    /// Almost all of this vocabulary is already log-safe by what it is about:
    /// a commit's own bookkeeping is generations, replica positions, attempt
    /// counts and control-object names, none of which anybody chose. The
    /// exceptions are [`EntryPathCollision`](Self::EntryPathCollision) and
    /// [`RebaseConflict`](Self::RebaseConflict), which are *identified* by a
    /// path — and either is enough on its own to make rendering this type with
    /// `Display` unsafe.
    fn redacted(&self) -> String {
        match self {
            Self::Storage(error) => format!("Commit::Storage: {}", error.redacted()),
            Self::Index(error) => format!("Commit::Index: {}", error.redacted()),
            Self::Format(error) => format!("Commit::Format: {}", error.redacted()),
            Self::EntryPathCollision { path } => format!(
                "Commit::EntryPathCollision(path_len={})",
                path.as_str().len()
            ),
            Self::RemovalNotCurrent { container_ids } => format!(
                "Commit::RemovalNotCurrent(containers=[{}])",
                listed(container_ids)
            ),
            Self::RebaseConflict {
                pending,
                record,
                rule,
                path,
            } => format!(
                "Commit::RebaseConflict(pending={}, record={record}, rule={rule:?}, \
                 path_len={})",
                pending.redacted(),
                path.as_str().len()
            ),
            Self::RenameRefused { element, rule } => format!(
                "Commit::RenameRefused(element={element}, rule={})",
                rule.redacted()
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
            Self::FirstHeadUnlisted => "Commit::FirstHeadUnlisted".to_owned(),
            Self::DuplicateControlObject {
                object,
                first,
                second,
            } => format!(
                "Commit::DuplicateControlObject(object={object}, first={first}, second={second})"
            ),
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
