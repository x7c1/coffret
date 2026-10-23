use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};

use coffret_format::{encode_keyring, keyring_set_digest};
use coffret_model::{
    ContainerId, ControlObjectName, Generation, KeyTable, KeyringCommitment, KeyringElement,
    ReplicaPosition,
};
use futures_util::future::join_all;
use tracing::debug;

use super::{read_replica, write_replica, Examined};
use crate::commit::commit_error::{CommitError, CommitResult};
use crate::commit::commit_policy::CommitPolicy;
use crate::commit::committing::Committing;
use crate::commit::control_keys::ControlKeys;
use crate::commit::prepared_batch::PreparedBatch;
use crate::index::Index;
use crate::object_store::ObjectStore;

/// Writes the Keyring generation this commit will select, and proves it
/// complete (spec: CP-8, CP-9, KL-2, KL-14).
///
/// The key table covers exactly the post-commit Container set
/// `(current − removals) ∪ additions`: the batch's own envelopes for what it
/// adds, and what the committed Keyring already held for everything that
/// survives — an envelope, or the key-lost marker that says the committed
/// control state has none (spec: KL-7). The previously committed generation
/// stays authoritative throughout, so excluding the removed Containers does not
/// make the pre-commit state unreadable (spec: CP-9).
///
/// Every replica is written unconditionally, for the reason [`write_replica`]
/// gives. Then every one of them is read back and checked (spec: KL-1): the
/// object opens, its header agrees with its name, and the digest of the key table
/// inside it is the digest its name carries. One that does not stops the flow —
/// a candidate set that is not complete is not one a commit may select
/// (spec: KL-2, CP-8), and what has already been written stays an uncommitted
/// candidate, which selects nothing (spec: KL-3).
///
/// The replicas are written all at once and read back all at once, because no
/// rule orders them. What CP-8 asks is that a complete set be written and
/// verified before the Journal commit, and completeness is which positions are
/// present rather than the order they arrived in (spec: KL-2). A replica's name
/// fixes its content, so a write made unconditionally, or twice, is harmless
/// (spec: KL-14). And an interrupted write leaves a partial candidate whatever
/// subset of it landed, which nothing selects and orphan cleanup treats as it
/// treats any other (spec: KL-3, KL-5, CP-9, KL-12, OC-2). Writing one after
/// another bought nothing for that and cost a round trip per replica.
///
/// The head still waits for every one of them: this returns only once every
/// position has been read back valid, and the commitment it returns is what the
/// head is written from (spec: CP-1, CP-8, CP-10). Where several positions
/// fail, the error names the lowest of them.
///
/// Each replica is reported to `committing` as it lands, in the order they
/// land (see [`Phase::Committing`](crate::Phase::Committing)).
///
/// What the committed generation held, and which number this one takes, are
/// [`examine`](super::examine)'s to say: the walk that reads the committed set is also the one
/// that repairs it, and reading it twice per commit would be paying the walk's
/// price for half of what it establishes.
pub(in crate::commit) async fn replicate(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    examined: &Examined,
    batch: &PreparedBatch,
    committing: Committing<'_>,
) -> CommitResult<KeyringCommitment> {
    let key_table = next_generation(index, &examined.held, batch).await?;
    write_generation(store, keys, policy, examined.next, &key_table, committing).await
}

/// Writes every replica of one Keyring generation and reads every one of them
/// back, returning the commitment a head selects it by (spec: CP-8, KL-2,
/// KL-14).
///
/// [`replicate`]'s writing half, apart because writing a Library's first head
/// writes a generation too — the empty key table a Library that holds nothing
/// maps — and has no Index to compute one from (spec: CP-2, FM-13).
pub(in crate::commit) async fn write_generation(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    generation: Generation,
    key_table: &KeyTable,
    committing: Committing<'_>,
) -> CommitResult<KeyringCommitment> {
    let digest = keyring_set_digest(key_table)?;
    let payload = encode_keyring(key_table, keys.master_key_epoch())?;

    let names = (0..policy.replica_count)
        .map(|index_of| {
            let replica = ReplicaPosition::new(index_of, policy.replica_count)?;
            Ok(ControlObjectName::keyring_replica(
                generation, &digest, replica,
            )?)
        })
        .collect::<CommitResult<Vec<_>>>()?;

    // Every write is made, even past one that fails: the futures are driven to
    // the end together, and what is reported is the lowest position that
    // failed, so the error does not depend on which write Storage answered
    // first.
    let stored = AtomicUsize::new(0);
    let writes = join_all(names.iter().map(|name| async {
        let object = write_replica(store, keys, &policy.retry, name, &payload).await?;
        committing.replicas_stored(stored.fetch_add(1, Ordering::Relaxed) + 1);
        Ok::<_, CommitError>(object)
    }))
    .await;
    let written = writes.into_iter().collect::<CommitResult<Vec<_>>>()?;

    let reads = join_all(
        names
            .iter()
            .zip(&written)
            .map(|(name, object)| read_replica(store, keys, &policy.retry, name, object, &digest)),
    )
    .await;
    for (index_of, read) in (0u16..).zip(reads) {
        read.map_err(|cause| CommitError::IncompleteKeyring {
            generation,
            replica: index_of,
            cause,
        })?;
    }

    debug!(
        generation = generation.get(),
        replicas = policy.replica_count,
        containers = key_table.elements().len(),
        "the candidate Keyring is complete",
    );
    Ok(KeyringCommitment::new(
        generation,
        policy.replica_count,
        &digest,
    )?)
}

/// The key table the next generation carries (spec: CP-8, KL-7).
///
/// The Containers a device currently catalogs come from the Index rather than
/// from a listing of Storage, because which Containers are *current* is what a
/// committed Journal says and not what happens to be lying in the bucket
/// (spec: CP-1, OC-1).
async fn next_generation(
    index: &dyn Index,
    held: &KeyTable,
    batch: &PreparedBatch,
) -> CommitResult<KeyTable> {
    let removed: BTreeSet<ContainerId> = batch.removals.iter().copied().collect();
    let held: BTreeMap<ContainerId, KeyringElement> = held
        .elements()
        .iter()
        .map(|element| (element.container_id, *element))
        .collect();

    let mut elements = Vec::new();
    // A Container is reported through the Entries it holds, which is every
    // Container a commit can produce: one is built out of Entries and a batch
    // that adds none adds no Container either (spec: PK-1, PK-15).
    for container in index.containers_under(None).await? {
        if removed.contains(&container.id) {
            continue;
        }
        let element = held
            .get(&container.id)
            .copied()
            // KL-7 says the committed Keyring maps every current Container to
            // an envelope or an explicit marker. One it says nothing about
            // leaves nothing to carry over, and minting a key-lost marker here
            // would record a loss the Library never suffered.
            .ok_or(CommitError::UnmappedContainer {
                container_id: container.id,
            })?;
        elements.push(element);
    }
    for prepared in &batch.additions {
        elements.push(KeyringElement::envelope(
            prepared.addition.container().id,
            prepared.envelope,
        ));
    }
    // The Containers arrive in the order the Index reported them and the
    // batch's additions after them, so the key table is put in the order FM-17
    // fixes here — and a Container the batch re-added while the held key table
    // still listed it is refused now, at the writer, rather than written for
    // every reader to reject (spec: FM-17, KL-7).
    KeyTable::canonical(elements).map_err(|cause| CommitError::UnwritableControlValue { cause })
}
