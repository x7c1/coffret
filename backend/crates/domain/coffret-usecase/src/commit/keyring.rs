use std::collections::{BTreeMap, BTreeSet};

use coffret_format::{
    decode_keyring, encode_control_object, encode_keyring, keyring_set_digest,
    ControlEncodeRequest, ControlPayload,
};
use coffret_model::{
    ContainerId, ControlObjectKind, ControlObjectName, Generation, KeyringCommitment, KeyringEntry,
    KeyringMapping, ObjectRef, ReplicaPosition,
};
use tracing::{debug, warn};

use crate::byte_stream::ByteStream;
use crate::commit::commit_error::{CommitError, CommitResult, InvalidReplica, UnrepairedReplica};
use crate::commit::commit_policy::CommitPolicy;
use crate::commit::control_keys::ControlKeys;
use crate::commit::control_listing::ControlListing;
use crate::commit::control_object;
use crate::commit::keyring_repair::KeyringRepair;
use crate::commit::prepared_batch::PreparedBatch;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// The committed Keyring as this commit found it, once it had been repaired.
///
/// The three things the rest of the flow needs out of that one walk, kept
/// together because they come from it together: the mapping the next generation
/// carries forward (spec: KL-7), the number that generation takes (spec: KL-10),
/// and what the repair did, for the caller KL-15 obliges to hear about it.
pub(super) struct Examined {
    /// The mapping the committed generation holds, from any one valid replica
    /// (spec: KL-6).
    held: KeyringMapping,
    /// The generation this commit will prepare.
    next: Generation,
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
    pub(super) fn first() -> Self {
        Self {
            held: KeyringMapping::default(),
            next: Generation::FIRST,
            repair: None,
        }
    }

    /// The repair this examination performed, taken out for [`CommitOutcome`]
    /// to carry.
    ///
    /// Taken rather than read at the end of the attempt because the run keeps
    /// every repair it performs and not only the last one: an attempt that put
    /// a position back and then lost the commit slot did that work all the
    /// same, and the rebase behind it examines whatever set the winner
    /// committed (spec: CP-4, KL-15).
    ///
    /// [`CommitOutcome`]: super::CommitOutcome
    pub(super) fn take_repair(&mut self) -> Option<KeyringRepair> {
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
/// (spec: KL-13): the mapping comes from a committed valid replica, is encoded
/// exactly as [`replicate`] encodes a generation, and goes to the name the
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
/// What the refusal carries is the first position that could not be completed —
/// the rest are in `needed` beside it, and the positions this walk did put back
/// are in `rewritten`, because a repair that stopped is still a repair
/// performed and there is no outcome left to report it on (spec: KL-15).
pub(super) async fn examine(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    listing: &ControlListing,
    commitment: &KeyringCommitment,
) -> CommitResult<Examined> {
    let generation = commitment.generation();
    let mut held: Option<KeyringMapping> = None;
    let mut walked: Vec<Walked> = Vec::new();

    for index_of in 0..commitment.replica_count() {
        let name = replica_name(commitment, index_of)?;
        let Some(object) = listing.handle(&name.to_string()) else {
            walked.push(Walked::at(
                index_of,
                name,
                Found::Lost(InvalidReplica::Absent),
            ));
            continue;
        };
        let read = read_replica(
            store,
            keys,
            &policy.retry,
            &name,
            object,
            commitment.set_digest(),
        )
        .await;
        walked.push(match read {
            Ok(mapping) => {
                held.get_or_insert(mapping);
                Walked::at(index_of, name, Found::Valid)
            }
            Err(InvalidReplica::Unfetchable(error)) => {
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
            .and_then(|last| last.found.invalid().map(|cause| (last.index_of, cause)))
            .unwrap_or((0, InvalidReplica::Absent));
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

    for position in walked {
        let refusal = match position.found {
            Found::Valid => continue,
            Found::Unfetchable(error) => Some(UnrepairedReplica::Unfetchable(error)),
            Found::Lost(_) => rewrite(
                store,
                keys,
                &policy.retry,
                &position.name,
                commitment.set_digest(),
                &payload,
            )
            .await
            .err(),
        };
        match refusal {
            Some(cause) => {
                needed.push(position.index_of);
                stopped.get_or_insert((position.index_of, cause));
            }
            None => rewritten.push(position.index_of),
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
            "the committed Keyring is degraded and the repair did not complete",
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
        repair: (!rewritten.is_empty()).then_some(KeyringRepair {
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
    Lost(InvalidReplica),
    /// Storage would not hand the object over, so nothing about it is known.
    Unfetchable(Box<CommitError>),
}

impl Found {
    /// The verdict as a read of the set would report it, or `None` where the
    /// replica was valid.
    fn invalid(self) -> Option<InvalidReplica> {
        match self {
            Self::Valid => None,
            Self::Lost(cause) => Some(cause),
            Self::Unfetchable(error) => Some(InvalidReplica::Unfetchable(error)),
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
/// still not one a mapping may be read from.
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

/// The name one position of a committed generation is stored under
/// (spec: FM-12).
fn replica_name(commitment: &KeyringCommitment, index_of: u16) -> CommitResult<ControlObjectName> {
    let replica = ReplicaPosition::new(index_of, commitment.replica_count())?;
    Ok(ControlObjectName::keyring_replica(
        commitment.generation(),
        commitment.set_digest(),
        replica,
    )?)
}

/// Writes the Keyring generation this commit will select, and proves it
/// complete (spec: CP-8, CP-9, KL-2, KL-14).
///
/// The mapping covers exactly the post-commit Container set
/// `(current − removals) ∪ additions`: the batch's own envelopes for what it
/// adds, and what the committed Keyring already held for everything that
/// survives — an envelope, or the key-lost marker that says the committed
/// control state has none (spec: KL-7). The previously committed generation
/// stays authoritative throughout, so excluding the removed Containers does not
/// make the pre-commit state unreadable (spec: CP-9).
///
/// Every replica is written unconditionally, for the reason [`write_replica`]
/// gives. Then every one of them is read back and checked (spec: KL-1): the
/// object opens, its header agrees with its name, and the digest of the mapping
/// inside it is the digest its name carries. One that does not stops the flow —
/// a candidate set that is not complete is not one a commit may select
/// (spec: KL-2, CP-8), and what has already been written stays an uncommitted
/// candidate, which selects nothing (spec: KL-3).
///
/// What the committed generation held, and which number this one takes, are
/// [`examine`]'s to say: the walk that reads the committed set is also the one
/// that repairs it, and reading it twice per commit would be paying the walk's
/// price for half of what it establishes.
pub(super) async fn replicate(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    examined: &Examined,
    batch: &PreparedBatch,
) -> CommitResult<KeyringCommitment> {
    let generation = examined.next;
    let mapping = next_generation(index, &examined.held, batch).await?;
    let digest = keyring_set_digest(&mapping)?;
    let payload = encode_keyring(&mapping, keys.master_key_epoch())?;

    let mut written: Vec<(ControlObjectName, ObjectRef)> = Vec::new();
    for index_of in 0..policy.replica_count {
        let replica = ReplicaPosition::new(index_of, policy.replica_count)?;
        let name = ControlObjectName::keyring_replica(generation, &digest, replica)?;
        let stored = write_replica(store, keys, &policy.retry, &name, &payload).await?;
        written.push((name, stored));
    }

    for (index_of, (name, object)) in written.iter().enumerate() {
        // The cast cannot lose: the loop above ran `replica_count` times, which
        // is a `u16`.
        let index_of = index_of as u16;
        read_replica(store, keys, &policy.retry, name, object, &digest)
            .await
            .map_err(|cause| CommitError::IncompleteKeyring {
                generation,
                replica: index_of,
                cause,
            })?;
    }

    debug!(
        generation = generation.get(),
        replicas = policy.replica_count,
        containers = mapping.entries().len(),
        "the candidate Keyring is complete",
    );
    Ok(KeyringCommitment::new(
        generation,
        policy.replica_count,
        &digest,
    )?)
}

/// The mapping the next generation carries (spec: CP-8, KL-7).
///
/// The Containers a device currently catalogs come from the Index rather than
/// from a listing of Storage, because which Containers are *current* is what a
/// committed Journal says and not what happens to be lying in the bucket
/// (spec: CP-1, OC-1).
async fn next_generation(
    index: &dyn Index,
    held: &KeyringMapping,
    batch: &PreparedBatch,
) -> CommitResult<KeyringMapping> {
    let removed: BTreeSet<ContainerId> = batch.removals.iter().copied().collect();
    let held: BTreeMap<ContainerId, KeyringEntry> = held
        .entries()
        .iter()
        .map(|entry| (entry.container_id, *entry))
        .collect();

    let mut entries = Vec::new();
    // A Container is reported through the Entries it holds, which is every
    // Container a commit can produce: one is built out of Entries and a batch
    // that adds none adds no Container either (spec: PK-1, PK-15).
    for container in index.containers_under(None).await? {
        if removed.contains(&container.id) {
            continue;
        }
        let entry = held
            .get(&container.id)
            .copied()
            // KL-7 says the committed Keyring maps every current Container to
            // an envelope or an explicit marker. One it says nothing about
            // leaves nothing to carry over, and minting a key-lost marker here
            // would record a loss the Library never suffered.
            .ok_or(CommitError::UnmappedContainer {
                container_id: container.id,
            })?;
        entries.push(entry);
    }
    for prepared in &batch.additions {
        entries.push(KeyringEntry::envelope(
            prepared.addition.container().id,
            prepared.envelope,
        ));
    }
    // The Containers arrive in the order the Index reported them and the
    // batch's additions after them, so the mapping is put in the order FM-17
    // fixes here — and a Container the batch re-added while the held mapping
    // still listed it is refused now, at the writer, rather than written for
    // every reader to reject (spec: FM-17, KL-7).
    KeyringMapping::canonical(entries)
        .map_err(|cause| CommitError::UnwritableControlValue { cause })
}

/// The mapping the committed Keyring holds, from any one valid replica
/// (spec: KL-1, KL-3, KL-6).
///
/// One valid replica carries the whole logical Keyring, so the replica count is
/// redundancy and never a quorum (spec: KL-6): the first one that reads back
/// valid answers, and the rest are not fetched. A replica Storage will not hand
/// over, one that does not open, or one whose mapping is not the one its name
/// promises, is stepped over and the walk goes on to the next position, so a
/// degraded set still serves a read (spec: RV-2). Only a generation no replica
/// of answers is refused, and whether that is the Keyring loss RV-7 names is
/// what the reason inside [`CommitError::KeyringUnreadable`] leaves to a caller
/// — which is why that reason keeps a fetch that failed and an object that was
/// rejected apart (see [`InvalidReplica`]).
///
/// A committed replica the walk had to step over is one this read could not
/// take the mapping from, and where the object is absent or would not open it
/// is one the set no longer has — fewer valid replicas than the count its
/// commitment selected, which is the degraded state KL-5 names. A read carries
/// on regardless (spec: RV-2), and repairs nothing: restoring the set is a
/// write, and it belongs to the flow that is about to write anyway
/// (spec: KL-11, KL-13), which is [`examine`]. So the degradation is worth a
/// line here, and the line says the set awaits a repair rather than promising
/// one this read performs. The count in it is a floor rather than a tally: the
/// replicas above the one that answered are never fetched. It counts every
/// position the walk stepped over, whatever the reason — which is why it is
/// named for the stepping rather than for any one of the verdicts.
///
/// Crate-visible, for the reason [`catch_up`](super::catch_up()) is: reading the
/// committed Keyring is not the commit's alone. A fetch reads it to open the
/// Containers it pulled back, and a freeze reads it to learn which Containers
/// have no key (spec: KL-7, RV-3) — the same routine, against the same walk of
/// Storage, answering the same rule. Two copies of it would be two readings of
/// KL-1. The commit reads the committed set through [`examine`] instead, which
/// is this walk made exhaustive and followed by the repair it is allowed to
/// perform; a commit that also called this one would log the degradation twice
/// for one finding.
pub(crate) async fn read_committed(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    retry: &RetryPolicy,
    listing: &ControlListing,
    commitment: &KeyringCommitment,
) -> CommitResult<KeyringMapping> {
    let mut last: Option<(u16, InvalidReplica)> = None;
    let mut stepped_over = 0u16;
    for index_of in 0..commitment.replica_count() {
        let replica = ReplicaPosition::new(index_of, commitment.replica_count())?;
        let name = ControlObjectName::keyring_replica(
            commitment.generation(),
            commitment.set_digest(),
            replica,
        )?;
        let Some(object) = listing.handle(&name.to_string()) else {
            last = Some((index_of, InvalidReplica::Absent));
            stepped_over += 1;
            continue;
        };
        match read_replica(store, keys, retry, &name, object, commitment.set_digest()).await {
            Ok(mapping) => {
                if stepped_over > 0 {
                    warn!(
                        generation = commitment.generation().get(),
                        replicas = commitment.replica_count(),
                        stepped_over,
                        "the committed Keyring is degraded and awaits repair by a writer",
                    );
                }
                return Ok(mapping);
            }
            Err(cause) => {
                last = Some((index_of, cause));
                stepped_over += 1;
            }
        }
    }
    // A commitment declares at least one replica (spec: KL-2), so the walk
    // above always leaves a verdict behind; the fallback is what keeps that
    // invariant from having to be unwrapped.
    let (replica, cause) = last.unwrap_or((0, InvalidReplica::Absent));
    Err(CommitError::KeyringUnreadable {
        generation: commitment.generation(),
        replica,
        cause,
    })
}

/// Frames one replica of a generation and writes it (spec: FM-11, KL-14).
///
/// The write is unconditional, because a replica at
/// `(generation, set_digest, index)` has exactly one valid content: two devices
/// writing that name write the same mapping under the same digest, so there is
/// no race whose loser would need reporting and a duplicate is benign. The
/// objects still differ, each sealed with a random nonce of its own — what is
/// identical is the content the digest covers.
///
/// Shared by the two writers of a Keyring replica, because they are writing the
/// same thing for different reasons: [`replicate`] materializes a candidate
/// generation, and [`rewrite`] re-materializes a position of a committed one
/// (spec: KL-13). A second copy of the framing would be a second reading of
/// what a replica object is.
async fn write_replica(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    retry: &RetryPolicy,
    name: &ControlObjectName,
    payload: &ControlPayload,
) -> CommitResult<ObjectRef> {
    let object = encode_control_object(&ControlEncodeRequest::new(
        name,
        ControlObjectKind::Keyring,
        keys.of_kind(ControlObjectKind::Keyring),
        payload,
    ))?;
    let spelling = name.to_string();
    Ok(retry
        .run("put", || {
            store.put(&spelling, ByteStream::from(object.bytes().to_vec()))
        })
        .await?)
}

/// Reads one replica back and decides whether it is valid (spec: KL-1).
///
/// Validity is three things and the framing already checks two of them: the
/// object opens and authenticates, and its header's kind, generation, and
/// replica position agree with the name it was fetched under (spec: FM-11,
/// FM-12). The third is this crate's to check — the digest of the mapping
/// inside is the digest the name carries — because that is what binds a name to
/// one content and a commitment to one mapping (spec: CP-10, KL-3, KL-14).
///
/// A failure comes back as an [`InvalidReplica`] rather than as a
/// [`CommitError`], because the caller is what knows whether it means "this
/// replica is unreadable, try the next one" or "the candidate set is
/// incomplete, stop". Which of the two it is decides the variant the reason
/// ends up in, and the reason itself travels as a value either way.
///
/// The fetch and the open are driven as two steps rather than through
/// [`control_object::read`], because the reason has to say which of them
/// failed: Storage refusing the object leaves the replica's content unknown,
/// while an object that arrived and would not open is a replica this Library
/// definitively cannot read a mapping from (spec: KL-1, KL-5). Neither decides
/// anything differently here — the caller's step-over or stop is the same
/// either way — so what the split changes is only how precise the value is.
async fn read_replica(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    retry: &RetryPolicy,
    name: &ControlObjectName,
    object: &ObjectRef,
    expected: &str,
) -> std::result::Result<KeyringMapping, InvalidReplica> {
    let bytes = control_object::fetch(store, retry, name, object)
        .await
        .map_err(|error| unfetchable(error.into()))?;
    let decoded = control_object::open(keys, name, &bytes).map_err(unreadable)?;
    if decoded.kind != ControlObjectKind::Keyring {
        return Err(InvalidReplica::KindNotAdmitted {
            found: decoded.kind,
        });
    }
    let mapping = decode_keyring(&decoded.payload).map_err(|error| unreadable(error.into()))?;
    let actual = keyring_set_digest(&mapping).map_err(|error| unreadable(error.into()))?;
    if actual != expected {
        return Err(InvalidReplica::DigestMismatch {
            expected: expected.to_owned(),
            actual,
        });
    }
    Ok(mapping)
}

/// What Storage reported, as the reason the replica never arrived.
fn unfetchable(error: CommitError) -> InvalidReplica {
    InvalidReplica::Unfetchable(Box::new(error))
}

/// What the format layer reported, as the reason the replica that did arrive is
/// not one a mapping may be read from.
fn unreadable(error: CommitError) -> InvalidReplica {
    InvalidReplica::Unreadable(Box::new(error))
}
