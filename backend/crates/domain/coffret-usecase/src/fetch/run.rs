use std::collections::BTreeMap;

use coffret_model::{
    ContainerId, ContainerKeyStatus, ContainerSummary, KeyEnvelope, KeyringMapping,
};
use tracing::info;

use crate::commit::{catch_up, read_committed};
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::fetch_outcome::FetchOutcome;
use crate::fetch::fetch_request::FetchRequest;
use crate::fetch::placement::publish_all;
use crate::fetch::surfaced::Surfaced;
use crate::fetch::target::Target;
use crate::fetch::{container, select, translate};

/// Materializes the Library's current Entries into this device's mapped folders.
///
/// The whole path, in the order it has to happen in: catch the Index up to the
/// Library's head (spec: CK-9), let the mappings say where each current Entry
/// would go (spec: EP-9), decide per Entry whether this device may write there
/// (spec: EP-10, EP-11), open the committed Keyring the caught-up checkpoint
/// names (spec: KL-1, KL-3), fetch each needed Container once (spec: PK-16),
/// verify what came back twice over (spec: FM-15, CP-11), and place the files
/// (spec: EP-11).
///
/// The catch-up comes first and its failure fails the run. A fetch is a claim
/// about what the Library currently holds, and one that served an Index it had
/// not brought to the head would be answering from a catalog it knew might be
/// stale — on a fresh device, from no catalog at all. That is also what lets a
/// second enrolled device fetch with an empty Index: catching up there is
/// restore-from-newest-checkpoint plus replay, and neither step opens a
/// Container (spec: CK-9, RV-1, RV-5).
///
/// Nothing is placed that has not been verified, and nothing is passed over
/// silently. Both halves are in the outcome: [`FetchOutcome::fetched`] is what
/// is now on disk, and [`FetchOutcome::surfaced`] is every Entry the run
/// declined and why. A run that returns successfully with findings in it has
/// *not* made the folder a copy of the Library (spec: EP-11).
///
/// A Container the committed Keyring records no key for is reported locked and
/// costs its own Entries and nothing else: the rest of the run fetches and
/// places as usual (spec: KL-7, KL-17, RV-2). What does stop the run is an
/// object that is not what the catalog says it is — the integrity verdicts are
/// about the Library's own consistency, not about one file's availability, and
/// carrying on past one would place files on the strength of a catalog that has
/// just been shown wrong.
pub async fn fetch_folders(request: FetchRequest<'_>) -> FetchResult<FetchOutcome> {
    let FetchRequest {
        store,
        index,
        keys,
        prefix,
        now,
        policy,
    } = request;

    let caught = catch_up(store, index, keys.control(), &policy.retry).await?;

    let mut outcome = FetchOutcome {
        fetched: Vec::new(),
        containers: Vec::new(),
        skipped: 0,
        surfaced: Vec::new(),
        locked: Vec::new(),
    };

    let Some(checkpoint) = index.checkpoint().await? else {
        // A Library that has committed nothing holds no current Entry, so there
        // is nothing to place and no Keyring to open (spec: CP-1, FM-13).
        finished(&outcome);
        return Ok(outcome);
    };

    let selection =
        select::select(index, translate::targets(index, prefix.as_ref()).await?).await?;
    outcome.skipped = selection.skipped;
    outcome.surfaced = selection.surfaced;
    if selection.wanted.is_empty() {
        finished(&outcome);
        return Ok(outcome);
    }

    // Read once for the whole run. One valid replica carries the whole Keyring,
    // so the count is redundancy and never a quorum (spec: KL-6).
    let keyring = read_committed(
        store,
        keys.control(),
        &policy.retry,
        &caught.listing,
        &checkpoint.keyring,
    )
    .await?;
    // Which Containers are current is what the Journal says rather than what a
    // listing happens to hold (spec: CP-1, OC-1), and the port answers a prefix
    // at a time. One walk under the run's own prefix covers every Container the
    // selection can name — a wanted Entry lies under that prefix by
    // construction — however many mappings overlap it and whichever Containers
    // their Entries turn out to share (spec: PK-8).
    let summaries: BTreeMap<ContainerId, ContainerSummary> = index
        .containers_under(prefix.as_ref())
        .await?
        .into_iter()
        .map(|container| (container.id, container))
        .collect();

    for (container_id, wanted) in grouped(selection.wanted) {
        let summary = summaries
            .get(&container_id)
            .ok_or(FetchError::ContainerUnreachable { container_id })?;
        let Some(envelope) = envelope(&keyring, container_id)? else {
            // Present but locked: the ciphertext stays where it is and the
            // Entries are reported rather than fetched (spec: KL-7, KL-17,
            // RV-2, RV-7).
            outcome.locked.push(container_id);
            outcome
                .surfaced
                .extend(wanted.into_iter().map(|target| Surfaced::KeyLost {
                    path: target.location.entry.path,
                    container_id,
                }));
            continue;
        };

        let placements = container::fetch(
            store,
            &policy.retry,
            keys,
            &caught.listing,
            summary,
            &envelope,
            &wanted,
        )
        .await?;
        outcome.containers.push(container_id);
        outcome
            .fetched
            .extend(publish_all(index, now, placements).await?);
    }

    // The Entries came out grouped by Container, and a caller reading a list of
    // paths wants them in the order the Library puts them in (spec: EP-3).
    outcome.fetched.sort_unstable();
    finished(&outcome);
    Ok(outcome)
}

/// The wanted Entries by the Container that holds them, so that each Container
/// is fetched exactly once however many of its Entries are wanted
/// (spec: PK-16).
fn grouped(wanted: Vec<Target>) -> BTreeMap<ContainerId, Vec<Target>> {
    let mut grouped: BTreeMap<ContainerId, Vec<Target>> = BTreeMap::new();
    for target in wanted {
        grouped
            .entry(target.location.container_id)
            .or_default()
            .push(target);
    }
    grouped
}

/// The envelope the committed Keyring maps one Container to, or `None` where it
/// records the key as lost (spec: KL-7).
///
/// A Container the mapping says nothing at all about is neither: KL-7 admits
/// exactly two answers at a commit boundary, and reading silence as a key-lost
/// marker would report a loss the Library never recorded.
pub(super) fn envelope(
    keyring: &KeyringMapping,
    container_id: ContainerId,
) -> FetchResult<Option<KeyEnvelope>> {
    let entry = keyring
        .entries
        .iter()
        .find(|entry| entry.container_id == container_id)
        .ok_or(FetchError::UnmappedContainer { container_id })?;
    Ok(match entry.key {
        ContainerKeyStatus::Envelope(envelope) => Some(envelope),
        ContainerKeyStatus::KeyLost => None,
    })
}

/// Records what the run came to, in counts and Container IDs alone.
fn finished(outcome: &FetchOutcome) {
    info!(
        fetched = outcome.fetched.len(),
        containers = outcome.containers.len(),
        skipped = outcome.skipped,
        surfaced = outcome.surfaced.len(),
        locked = outcome.locked.len(),
        "a fetch run finished",
    );
}
