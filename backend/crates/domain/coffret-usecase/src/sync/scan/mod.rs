use std::collections::BTreeMap;

use coffret_model::{ContainerId, ContainerKind};
use tracing::debug;

use crate::device_state::{DeviceTime, Mapping};
use crate::index::Index;
use crate::local_scan::{unavailable_roots, walk_mappings, RootState, Walked};
use crate::sync::survey::Survey;
use crate::sync::sync_error::SyncResult;

mod deletions;
use deletions::deletions;

mod examine;
use examine::examine;

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
/// is never reported as changed and never reported as deleted, because a
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
/// [`LocalError`]: crate::local_error::LocalError
pub(super) async fn scan(index: &dyn Index, now: DeviceTime) -> SyncResult<Survey> {
    let mappings = index.mappings().await?;
    let Walked { found, roots } = walk_mappings(&mappings).await?;

    for root in &roots {
        if let RootState::Stamp(identity) = &root.state {
            index
                .set_mapping(Mapping {
                    root_identity: Some(identity.clone()),
                    ..root.mapping.clone()
                })
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

    let mut survey = Survey::default();
    for source in found.values() {
        examine(index, &kinds, now, source, &mut survey).await?;
    }
    survey
        .deferred
        .extend(deletions(index, &roots, &found).await?);
    survey.unavailable = unavailable_roots(&roots);

    // Counts only: a prefix is an Entry Path component and a local root is a
    // local path, and neither may reach a log line.
    debug!(
        mappings = mappings.len(),
        files = found.len(),
        candidates = survey.candidates.len(),
        unchanged = survey.unchanged,
        deferred = survey.deferred.len(),
        unavailable = survey.unavailable.len(),
        "scanned the mapped folders",
    );
    Ok(survey)
}
