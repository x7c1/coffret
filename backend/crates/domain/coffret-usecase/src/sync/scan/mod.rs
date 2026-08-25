use std::collections::BTreeMap;

use coffret_model::{ContainerId, ContainerKind};
use tracing::debug;

use crate::device_state::DeviceTime;
use crate::index::Index;
use crate::local_scan::walk_mappings;
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
pub(super) async fn scan(index: &dyn Index, now: DeviceTime) -> SyncResult<Survey> {
    let mappings = index.mappings().await?;
    let found = walk_mappings(&mappings).await?;

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
        .extend(deletions(index, &mappings, &found).await?);

    debug!(
        mappings = mappings.len(),
        files = found.len(),
        candidates = survey.candidates.len(),
        unchanged = survey.unchanged,
        deferred = survey.deferred.len(),
        "scanned the mapped folders",
    );
    Ok(survey)
}
