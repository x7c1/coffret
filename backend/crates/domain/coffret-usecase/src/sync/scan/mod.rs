use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, ContainerKind};
use tracing::debug;

use crate::device_state::DeviceTime;
use crate::index::Index;
use crate::local_scan::{unavailable_roots, walk_mappings, RootState, UnknownBirths, Walked};
use crate::mapped_roots::MappedRoots;
use crate::sync::survey::Survey;
use crate::sync::sync_error::SyncResult;

mod deletions;
use deletions::deletions;

mod examine;
use examine::examine;

mod renames;
use renames::pair_renames;

/// Walks the device's mapped folders and works out what has to happen.
///
/// The comparison is the cheap one first and the expensive one only where it
/// changes the answer: a file whose length and modification time are what this
/// device last observed is not opened at all, and one whose observation has
/// moved is hashed and compared against its current Entry's own hash, so a file
/// that was touched costs a read and no upload (spec: EP-10).
///
/// What the scan may say about a path is bounded by whether this device ever
/// materialized it. A path with a current Entry and no local row is one this
/// device never put on disk, and a mapping covering it does not change that: it
/// is never reported as modified and never reported as deleted, because a
/// deletion is a fact about a file this device placed (spec: EP-9, EP-10).
///
/// It is bounded a second way by whether the device can vouch for the mapped
/// root at all (spec: EP-12). A root that is not there, or one that is empty and
/// standing on a filesystem the mapping does not record, is unavailable: nothing
/// under it was walked, nothing under it is reported as deleted, and the mapping
/// is reported instead. A root the walk found on a filesystem the mapping does
/// not record while it *held* files is stamped with what the walk saw, which is
/// this step's own write rather than the walk's — [`LocalError`] has no
/// vocabulary for a port failure and must not grow one for this, while this step
/// already writes through the port for the observations it refreshes.
///
/// A file this device materialized that is gone, and a file that appeared under
/// the same mapped root holding exactly its Entry's content, are one file
/// renamed where nothing else the scan knows of holds that content: the pair
/// becomes a rename of the Entry rather than a deletion and an upload, held to
/// the rules a rename is (spec: EP-10, FM-15).
///
/// A file a drop reserved for a Pack is not this scan's to carry in
/// (spec: PK-17): it is neither added nor updated, and it is reported in
/// [`Survey::reserved`] as waiting for its Pack rather than passed over. It is
/// still a file the walk found, so a file this device placed is not read as
/// deleted for standing there reserved.
///
/// [`LocalError`]: crate::local_error::LocalError
pub(super) async fn scan(
    index: &dyn Index,
    roots: &dyn MappedRoots,
    births: &UnknownBirths,
    now: DeviceTime,
) -> SyncResult<Survey> {
    let mappings = index.mappings().await?;
    let Walked {
        found,
        roots: walked,
        stale,
    } = walk_mappings(roots, &mappings, births).await?;

    for root in &walked {
        if let RootState::Stamp(identity) = &root.state {
            index
                .set_mapping(root.mapping.clone().stamped(identity.clone()))
                .await?;
        }
    }

    // The kind decides which path a changed file takes, and the port answers
    // kinds a prefix at a time. One walk of the whole current set answers every
    // lookup below, however many mappings overlap it and whichever Containers
    // their Entries turn out to share (spec: PK-8).
    let kinds: BTreeMap<ContainerId, ContainerKind> = index
        .containers_under(None)
        .await?
        .into_iter()
        .map(|container| (container.id, container.kind))
        .collect();

    let reserved: BTreeSet<_> = index
        .reservations()
        .await?
        .into_iter()
        .flat_map(|reservation| reservation.paths)
        .collect();

    let mut survey = Survey::default();
    for source in found.values() {
        if reserved.contains(&source.path) {
            survey.reserved.push(source.path.clone());
            continue;
        }
        // The mapping a file was found under is the one whose root it stands in,
        // and its expected identity is what a move out of that root is held to
        // (spec: EP-13).
        let expected = walked
            .iter()
            .find(|root| root.mapping.local_root == source.root)
            .and_then(|root| root.mapping.expected_root_id);
        examine(index, roots, &kinds, now, source, expected, &mut survey).await?;
    }
    let deletions = deletions(index, &walked, &found, &mut survey).await?;
    // Once both halves are known: what is gone, and what appeared (spec: EP-10).
    pair_renames(index, roots, deletions, &mut survey).await?;
    survey.unavailable = unavailable_roots(&walked);
    survey.stale = stale;

    // Counts only: a prefix is an Entry Path component and a local root is a
    // local path, and neither may reach a diagnostic event.
    debug!(
        mappings = mappings.len(),
        files = found.len(),
        candidates = survey.candidates.len(),
        renames = survey.renames.len(),
        unchanged = survey.unchanged,
        reserved = survey.reserved.len(),
        surfaced = survey.surfaced.len(),
        departed = survey.departed.len(),
        forgotten = survey.forgotten.len(),
        unavailable = survey.unavailable.len(),
        stale = survey.stale.len(),
        "scanned the mapped folders",
    );
    Ok(survey)
}
