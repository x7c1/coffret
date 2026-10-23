use futures_util::future::join_all;
use tracing::warn;

use coffret_format::{encode_keyring, ControlPayload};
use coffret_model::{ControlObjectName, Generation, KeyTable, KeyringCommitment};

use super::{read_replica, replica_name, write_replica};
use crate::commit::commit_error::{CommitError, CommitResult, UnrepairedReplica, UnusableReplica};
use crate::commit::commit_policy::CommitPolicy;
use crate::commit::control_keys::ControlKeys;
use crate::commit::control_listing::ControlListing;
use crate::commit::keyring_repair::KeyringRepair;
use crate::commit::rewritten_replicas::RewrittenReplicas;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// The committed Keyring as this commit found it, once it had been repaired.
///
/// The three things the rest of the flow needs out of that one walk, kept
/// together because they come from it together: the key table the next generation
/// carries forward (spec: KL-7), the number that generation takes (spec: KL-10),
/// and what the repair did, for the caller KL-15 obliges to hear about it.
pub(in crate::commit) struct Examined {
    /// The key table the committed generation holds, from any one valid replica
    /// (spec: KL-6).
    pub(super) held: KeyTable,
    /// The generation this commit will prepare.
    pub(super) next: Generation,
    /// The repair this examination performed, or `None` where it performed
    /// none — a set found complete, or no committed set to examine at all.
    repair: Option<KeyringRepair>,
}

impl Examined {
    /// What a Library that has committed nothing starts from (spec: FM-13).
    ///
    /// No committed head means no committed Keyring, so the first generation is
    /// built from the batch alone — and there is nothing to examine, nothing to
    /// repair, and nothing for the outcome to report.
    pub(in crate::commit) fn first() -> Self {
        Self {
            held: KeyTable::default(),
            next: Generation::FIRST,
            repair: None,
        }
    }

    /// The repair this examination performed, taken out for the run to carry on
    /// [`CommitOutcome`] or [`CommitFailure`], whichever it ends in.
    ///
    /// Taken rather than read at the end of the attempt because the run keeps
    /// every repair it performs and not only the last one: an attempt that put
    /// a position back and then lost the commit slot did that work all the
    /// same, and the rebase behind it examines whatever set the winner
    /// committed (spec: CP-4, KL-15).
    ///
    /// [`CommitOutcome`]: crate::commit::CommitOutcome
    /// [`CommitFailure`]: crate::commit::CommitFailure
    pub(in crate::commit) fn take_repair(&mut self) -> Option<KeyringRepair> {
        self.repair.take()
    }
}

/// Examines the committed Keyring and repairs it, before this commit writes a
/// generation of its own (spec: KL-11, KL-13, KL-14, KL-16).
///
/// This is KL-11's "before another write". The walk is a full one — every
/// position the commitment declares, not only up to the first that answers —
/// because the alternative cannot see a set that has lost its *last* replica,
/// and a present-but-unreadable replica cannot be found any other way at all.
/// It costs a read per declared replica on every commit, and that cost is the
/// price of the rule.
///
/// A position is in need of repair when the listing does not hold its name or
/// the object does not read back valid (spec: KL-1). One Storage merely failed
/// to hand over is not among them: nothing about its content is known, so it is
/// not known to be lost, and rewriting it on that evidence would be inventing a
/// verdict. It still keeps the set from being complete, so it holds the gate
/// closed like any other short position (spec: KL-16).
///
/// The repair itself re-materializes the committed generation and nothing else
/// (spec: KL-13): the key table comes from a committed valid replica, is encoded
/// exactly as [`replicate`](super::replicate()) encodes a generation, and goes to the name the
/// commitment's own `(generation, set_digest, index)` gives it. Nothing is
/// deleted and no position that read back valid is written. A generation no
/// replica of answers is not a repair at all but the Keyring loss RV-7 names,
/// and it comes back as [`CommitError::KeyringUnreadable`], exactly as a read
/// of it would.
///
/// Every position in need is attempted, rather than stopping at the first that
/// fails. A provider that refuses one write will likely refuse the next, but a
/// set left one replica short is strictly better for the next run than one left
/// two short, and the writes are cheap next to the reads this walk already made.
/// The reads go out together and so do the repairs, for the reasons
/// [`replicate`](super::replicate()) gives for the candidate set; the verdicts are still taken in
/// position order. What the refusal carries is the first position that could
/// not be completed — the rest are in `needed` beside it, and the positions this
/// walk did put back are in `rewritten`, because a repair that stopped is still
/// a repair performed and there is no outcome left to report it on
/// (spec: KL-15).
pub(in crate::commit) async fn examine(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    listing: &ControlListing,
    commitment: &KeyringCommitment,
) -> CommitResult<Examined> {
    let generation = commitment.generation();
    let names = (0..commitment.replica_count())
        .map(|index_of| replica_name(commitment, index_of))
        .collect::<CommitResult<Vec<_>>>()?;

    // Every position is read at once: the walk is a full one whatever answers,
    // so nothing a read finds decides whether the next is made.
    let reads = join_all(names.iter().map(|name| async move {
        let object = listing
            .handle(&name.to_string())
            .ok_or(UnusableReplica::Absent)?;
        read_replica(
            store,
            keys,
            &policy.retry,
            name,
            object,
            commitment.set_digest(),
        )
        .await
    }))
    .await;

    let mut held: Option<KeyTable> = None;
    let mut walked: Vec<Walked> = Vec::new();
    for (index_of, (name, read)) in (0u16..).zip(names.into_iter().zip(reads)) {
        walked.push(match read {
            Ok(key_table) => {
                held.get_or_insert(key_table);
                Walked::at(index_of, name, Found::Valid)
            }
            Err(UnusableReplica::Unfetchable(error)) => {
                Walked::at(index_of, name, Found::Unfetchable(error))
            }
            Err(cause) => Walked::at(index_of, name, Found::Lost(cause)),
        });
    }

    let Some(held) = held else {
        // A commitment declares at least one replica (spec: KL-2), so the walk
        // above always leaves a verdict behind; the fallback is what keeps that
        // invariant from having to be unwrapped.
        let (replica, cause) = walked
            .pop()
            .and_then(|last| last.found.unusable().map(|cause| (last.index_of, cause)))
            .unwrap_or((0, UnusableReplica::Absent));
        return Err(CommitError::KeyringUnreadable {
            generation,
            replica,
            cause,
        });
    };

    let payload = encode_keyring(&held, keys.master_key_epoch())?;
    let mut rewritten = Vec::new();
    let mut needed = Vec::new();
    let mut stopped: Option<(u16, UnrepairedReplica)> = None;

    // The outcomes are taken in position order, so the refusal names the same
    // position however the writes interleaved.
    let payload = &payload;
    let repairs = join_all(walked.into_iter().map(|position| async move {
        let refusal = match position.found {
            Found::Valid => return None,
            Found::Unfetchable(error) => Some(UnrepairedReplica::Unfetchable(error)),
            Found::Lost(_) => rewrite(
                store,
                keys,
                &policy.retry,
                &position.name,
                commitment.set_digest(),
                payload,
            )
            .await
            .err(),
        };
        Some((position.index_of, refusal))
    }))
    .await;

    for (index_of, refusal) in repairs.into_iter().flatten() {
        match refusal {
            Some(cause) => {
                needed.push(index_of);
                stopped.get_or_insert((index_of, cause));
            }
            None => rewritten.push(index_of),
        }
    }

    if let Some((replica, cause)) = stopped {
        // The event the refusal cannot be: what reaches the person who asked
        // for the run is a sentence on their terminal, which nothing writes
        // down. Counts, positions and a generation, and nothing else
        // (spec: EL-1, EL-3, KL-15).
        warn!(
            generation = generation.get(),
            replicas = commitment.replica_count(),
            needed = needed.len(),
            rewritten = rewritten.len(),
            replica,
            "the committed Keyring set is short and the repair did not complete",
        );
        return Err(CommitError::UnrepairedKeyring {
            generation,
            needed,
            rewritten,
            replica,
            cause,
        });
    }
    if !rewritten.is_empty() {
        warn!(
            generation = generation.get(),
            replicas = commitment.replica_count(),
            rewritten = rewritten.len(),
            positions = ?rewritten,
            "replicas of the committed Keyring were missing or unreadable and were rewritten",
        );
    }

    Ok(Examined {
        held,
        next: generation.next()?,
        // A walk that put nothing back performed no repair, and a repair naming
        // no position is not what a set found complete leaves behind: the run's
        // outcome says nothing about the Keyring instead (spec: KL-15).
        repair: RewrittenReplicas::new(rewritten).map(|rewritten| KeyringRepair {
            generation,
            rewritten,
        }),
    })
}

/// One position the commitment declares, as the walk left it.
///
/// The name is kept rather than derived again where the repair needs it: it was
/// built to read the position and it is the same name the rewrite goes to —
/// deriving it twice would leave two places that have to agree about where a
/// replica lives (spec: FM-12).
struct Walked {
    index_of: u16,
    name: ControlObjectName,
    found: Found,
}

impl Walked {
    fn at(index_of: u16, name: ControlObjectName, found: Found) -> Self {
        Self {
            index_of,
            name,
            found,
        }
    }
}

/// What the walk found at one position the commitment declares.
enum Found {
    /// A valid replica (spec: KL-1).
    Valid,
    /// Nothing, or something that is definitively not a replica of this
    /// generation: a position the set has lost, and one a repair may rewrite
    /// (spec: KL-5, KL-13).
    Lost(UnusableReplica),
    /// Storage would not hand the object over, so nothing about it is known.
    Unfetchable(Box<CommitError>),
}

impl Found {
    /// The verdict as a read of the set would report it, or `None` where the
    /// replica was valid.
    fn unusable(self) -> Option<UnusableReplica> {
        match self {
            Self::Valid => None,
            Self::Lost(cause) => Some(cause),
            Self::Unfetchable(error) => Some(UnusableReplica::Unfetchable(error)),
        }
    }
}

/// Re-materializes one position of the committed generation, and confirms it
/// (spec: KL-13, KL-14).
///
/// The write is the unconditional one every replica write is, for the reason
/// [`write_replica`] gives, and the read-back after it is the same KL-1 check
/// the candidate set's read-back makes. What the two failures say apart is what
/// the caller reports: Storage refusing the write leaves the position exactly as
/// it was, while a read-back that refuses what came back says the position is
/// still not one a key table may be read from.
async fn rewrite(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    retry: &RetryPolicy,
    name: &ControlObjectName,
    expected: &str,
    payload: &ControlPayload,
) -> std::result::Result<(), UnrepairedReplica> {
    let object = write_replica(store, keys, retry, name, payload)
        .await
        .map_err(|error| UnrepairedReplica::Unwritten(Box::new(error)))?;
    read_replica(store, keys, retry, name, &object, expected)
        .await
        .map(|_| ())
        .map_err(UnrepairedReplica::Unconfirmed)
}
