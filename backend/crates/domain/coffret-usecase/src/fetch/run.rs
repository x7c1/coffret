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
use crate::fetch::reading::Reading;
use crate::fetch::surfaced::Surfaced;
use crate::fetch::target::Target;
use crate::fetch::{container, select, translate};
use crate::progress::{Phase, Step};
use crate::refused_root::RefusedRoot;

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
/// silently. Every half is in the outcome: [`FetchOutcome::fetched`] is what is
/// now on disk, [`FetchOutcome::surfaced`] is every Entry the run declined and
/// why, and [`FetchOutcome::refused`] is every mapping whose root is not the
/// root it was recorded against. A run that returns successfully with findings
/// in it has *not* made the folder a copy of the Library (spec: EP-11, EP-13).
///
/// A mapped root that will not vouch for itself costs the mappings recorded
/// against it and nothing else, the way a locked Container costs its own
/// Entries: the check happens where the placement happens, on the root handle
/// the write would have gone through, so the refusal arrives once per mapping
/// and the device's mappings standing elsewhere are placed into as usual
/// (spec: EP-13).
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
        destinations,
        prefix,
        now,
        progress,
        policy,
    } = request;

    // Said before it starts rather than counted through it: what the catch-up
    // has to replay is known only as it is read, and a fetch straight after a
    // join replays the whole Journal before the first Container is asked for.
    progress.step(Step::begun(Phase::CatchingUp));
    let caught = catch_up(store, index, keys.control(), &policy.retry).await?;

    let mut outcome = FetchOutcome {
        fetched: Vec::new(),
        containers: Vec::new(),
        skipped: 0,
        // Read here rather than taken from the selection, because it is an
        // answer about the device and not about what this run selected: a
        // device that maps nothing selects nothing whatever the Library holds,
        // and those are two different empty answers (spec: EP-9).
        mappings: index.mappings().await?.len(),
        surfaced: Vec::new(),
        refused: Vec::new(),
        locked: Vec::new(),
    };

    let Some(checkpoint) = index.checkpoint().await? else {
        // A Library that has committed nothing holds no current Entry, so there
        // is nothing to place and no Keyring to open (spec: CP-1, FM-13).
        finished(&outcome);
        return Ok(outcome);
    };

    // Which Entries this device may write, and where: every one of them is a
    // question about a path on disk, and how many there are is what this step
    // is finding out.
    progress.step(Step::begun(Phase::Scanning));
    let selection = select::select(
        index,
        destinations,
        translate::targets(index, prefix.as_ref()).await?,
    )
    .await?;
    outcome.skipped = selection.skipped;
    outcome.surfaced = selection.surfaced;
    if selection.wanted.is_empty() {
        finished(&outcome);
        return Ok(outcome);
    }

    // Read once for the whole run. One valid replica carries the whole Keyring,
    // so the count is redundancy and never a quorum (spec: KL-6).
    // Reported here and not held: a fetch writes nothing, so nothing later in
    // this run examines the set the mapping came from.
    let keyring = read_committed(
        store,
        keys.control(),
        &policy.retry,
        &caught.listing,
        checkpoint.keyring(),
    )
    .await?
    .reporting();
    // What every Container of this run is read against, which does not change
    // between them.
    let reading = Reading {
        store,
        retry: &policy.retry,
        keys,
        destinations,
        listing: &caught.listing,
    };
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

    let wanted_containers = grouped(selection.wanted);
    // Said before the first Container is read, so that a caller showing this
    // has a line up while the first — and perhaps largest — object travels.
    let total = wanted_containers.len();
    progress.step(Step::new(Phase::Fetching, 0, total));

    for (done, (container_id, wanted)) in wanted_containers.into_iter().enumerate() {
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
            // A Container nothing can open is one fewer to wait for, so it
            // counts towards the whole the way a fetched one does.
            progress.step(Step::new(Phase::Fetching, done + 1, total));
            continue;
        };

        let placed = container::fetch(&reading, summary, &envelope, &wanted).await?;
        outcome.containers.push(container_id);
        note_refusals(&mut outcome.refused, placed.refused);
        outcome
            .fetched
            .extend(publish_all(index, now, placed.placements).await?);
        progress.step(Step::new(Phase::Fetching, done + 1, total));
    }

    // The Entries came out grouped by Container, and a caller reading a list of
    // paths wants them in the order the Library puts them in (spec: EP-3).
    outcome.fetched.sort_unstable();
    finished(&outcome);
    Ok(outcome)
}

/// Adds the mappings one Container's placing would not touch, once each
/// (spec: EP-13).
///
/// Containers are fetched one after another and several of them may hold Entries
/// under the same refused mapping, so the run keeps the *first* refusal for each
/// mapping: the refusal is a fact about the mapping rather than about the
/// Container that happened to meet it, and reporting one mapping several times
/// would say nothing the first one did not.
///
/// A mapping is its prefix, because that is the key EP-9 bounds — one
/// Library-root mapping and one per top-level component — and the half of the
/// refusal EP-13 asks it to name. The folder is deliberately not the key: two
/// top-level components may be recorded against one folder, and a run that
/// collapsed them would name one mapping and stay silent about the other, so a
/// person who recorded the named one again would walk straight into the one
/// nothing had mentioned.
fn note_refusals(held: &mut Vec<RefusedRoot>, found: Vec<RefusedRoot>) {
    for root in found {
        if !held.iter().any(|seen| seen.prefix == root.prefix) {
            held.push(root);
        }
    }
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
        .entries()
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
        // A count and nothing else: a mapping names a prefix and a local root,
        // and neither may reach a diagnostic event.
        mappings = outcome.mappings,
        surfaced = outcome.surfaced.len(),
        refused_roots = outcome.refused.len(),
        locked = outcome.locked.len(),
        "a fetch run finished",
    );
}
