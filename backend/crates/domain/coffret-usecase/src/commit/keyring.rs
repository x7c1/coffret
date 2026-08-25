use std::collections::{BTreeMap, BTreeSet};

use coffret_format::{
    decode_keyring, encode_control_object, encode_keyring, keyring_set_digest, ControlEncodeRequest,
};
use coffret_model::{
    ContainerId, ControlObjectKind, ControlObjectName, Generation, IndexCheckpoint,
    KeyringCommitment, KeyringEntry, KeyringMapping, ObjectRef, ReplicaPosition,
};
use tracing::{debug, warn};

use crate::byte_stream::ByteStream;
use crate::commit::commit_error::{CommitError, CommitResult, InvalidReplica};
use crate::commit::commit_policy::CommitPolicy;
use crate::commit::control_keys::ControlKeys;
use crate::commit::control_listing::ControlListing;
use crate::commit::control_object;
use crate::commit::prepared_batch::PreparedBatch;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

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
/// Every replica is written unconditionally, because a replica at
/// `(generation, set_digest, index)` has exactly one valid content and two
/// writers preparing it write the same mapping (spec: KL-14). Then every one of
/// them is read back and checked (spec: KL-1): the object opens, its header
/// agrees with its name, and the digest of the mapping inside it is the digest
/// its name carries. One that does not stops the flow — a candidate set that is
/// not complete is not one a commit may select (spec: KL-2, CP-8), and what has
/// already been written stays an uncommitted candidate, which selects nothing
/// (spec: KL-3).
pub(super) async fn replicate(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    listing: &ControlListing,
    committed: Option<&IndexCheckpoint>,
    batch: &PreparedBatch,
) -> CommitResult<KeyringCommitment> {
    let held = match committed {
        Some(checkpoint) => {
            read_committed(store, keys, &policy.retry, listing, &checkpoint.keyring).await?
        }
        // A Library with no committed head has no committed Keyring either, and
        // its first generation is built from the batch alone (spec: FM-13).
        None => KeyringMapping::default(),
    };
    let generation = match committed {
        Some(checkpoint) => checkpoint.keyring.generation().next()?,
        None => Generation::FIRST,
    };

    let mapping = next_generation(index, &held, batch).await?;
    let digest = keyring_set_digest(&mapping)?;
    let payload = encode_keyring(&mapping, keys.master_key_epoch())?;

    let mut written: Vec<(ControlObjectName, ObjectRef)> = Vec::new();
    for index_of in 0..policy.replica_count {
        let replica = ReplicaPosition::new(index_of, policy.replica_count)?;
        let name = ControlObjectName::keyring_replica(generation, &digest, replica)?;
        let object = encode_control_object(&ControlEncodeRequest::new(
            &name,
            ControlObjectKind::Keyring,
            keys.of_kind(ControlObjectKind::Keyring),
            &payload,
        ))?;
        let spelling = name.to_string();
        let stored = policy
            .retry
            .run("put", || {
                store.put(&spelling, ByteStream::from(object.bytes().to_vec()))
            })
            .await?;
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
        containers = mapping.entries.len(),
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
        .entries
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
            prepared.addition.container.id,
            prepared.envelope,
        ));
    }
    Ok(KeyringMapping::new(entries))
}

/// The mapping the committed Keyring holds, from any one valid replica
/// (spec: KL-1, KL-3, KL-6).
///
/// One valid replica carries the whole logical Keyring, so the replica count is
/// redundancy and never a quorum (spec: KL-6): the first one that reads back
/// valid answers, and the rest are not fetched. A replica that does not open,
/// or whose mapping is not the one its name promises, is stepped over and the
/// walk goes on to the next position, so a degraded set still serves a read
/// (spec: RV-2). Only a generation no replica of answers is refused, and
/// whether that is the Keyring loss RV-7 names is what the reason inside
/// [`CommitError::KeyringUnreadable`] leaves to a caller.
///
/// A committed replica the walk had to step over means the set has fewer valid
/// replicas than the count its commitment selected, which is the degraded state
/// KL-5 names. A read carries on — a commit that calls this is preparing a
/// generation of its own, complete before it may be selected (spec: CP-8,
/// KL-2) — but the degradation is worth a line, because repairing the committed
/// set is a device's obligation (spec: KL-13) and neither flow that reads a
/// Keyring is what performs it. The count in that line is a floor rather than a
/// tally: the replicas above the one that answered are never fetched.
///
/// Crate-visible, for the reason [`catch_up`](super::catch_up) is: reading the
/// committed Keyring is not the commit's alone. A commit reads it to carry the
/// generation forward, and a fetch reads it to open the Containers it fetched
/// (spec: KL-7, RV-3) — the same routine, against the same walk of Storage,
/// answering the same rule. Two copies of it would be two readings of KL-1.
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
                        unreadable = stepped_over,
                        "the committed Keyring is degraded and awaits repair",
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
async fn read_replica(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    retry: &RetryPolicy,
    name: &ControlObjectName,
    object: &ObjectRef,
    expected: &str,
) -> std::result::Result<KeyringMapping, InvalidReplica> {
    let decoded = control_object::read(store, retry, keys, name, object)
        .await
        .map_err(unreadable)?;
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

/// What a layer below reported, as the reason a replica did not answer.
fn unreadable(error: CommitError) -> InvalidReplica {
    InvalidReplica::Unreadable(Box::new(error))
}
