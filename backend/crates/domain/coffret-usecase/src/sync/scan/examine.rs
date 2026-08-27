use std::collections::BTreeMap;

use coffret_model::{ContainerId, ContainerKind, ContentHash};

use crate::device_state::{DeviceTime, LocalEntryState, LocalObservation};
use crate::index::Index;
use crate::local_scan::SourceFile;
use crate::sync::candidate::Candidate;
use crate::sync::surfaced::Surfaced;
use crate::sync::survey::Survey;
use crate::sync::sync_error::SyncResult;

/// Decides what one local file means for the Library.
pub(super) async fn examine(
    index: &dyn Index,
    kinds: &BTreeMap<ContainerId, ContainerKind>,
    now: DeviceTime,
    source: &SourceFile,
    survey: &mut Survey,
) -> SyncResult<()> {
    let Some(location) = index.entry_at(&source.path).await? else {
        survey.candidates.push(Candidate {
            source: source.clone(),
            replaces: None,
        });
        return Ok(());
    };

    // A current Entry this device never materialized is outside its scope,
    // whether or not a mapping covers it (spec: EP-10). Reporting it as changed
    // would propose replacing an Entry from a file this device never put there.
    let Some(local) = index.local_entry_at(&source.path).await? else {
        return Ok(());
    };
    if local.state == LocalEntryState::Present
        && local.observation.size == source.size
        && local.observation.mtime == source.mtime
    {
        survey.unchanged += 1;
        return Ok(());
    }

    let content = source.read().await?;
    if ContentHash::from_bytes(*blake3::hash(&content).as_bytes()) == location.entry.hash {
        // Touched and not changed: the content the Library holds is still the
        // content on disk, so only what this device last saw of the file moves.
        survey.unchanged += 1;
        survey.refreshed.push(observation(source, now));
        return Ok(());
    }

    match kinds.get(&location.container_id) {
        Some(ContainerKind::OneFile) => survey.candidates.push(Candidate {
            source: source.clone(),
            replaces: Some(location.container_id),
        }),
        // Read-modify-replace over a Pack is the half of `update` this flow
        // does not do, and skipping the file quietly is what it may never do
        // instead (spec: PK-10, PK-11, PK-14).
        _ => survey.surfaced.push(Surfaced::PackResident {
            path: source.path.clone(),
            container_id: location.container_id,
        }),
    }
    Ok(())
}

/// What this device saw of a file, stamped with when it looked.
fn observation(source: &SourceFile, at: DeviceTime) -> LocalObservation {
    LocalObservation {
        path: source.path.clone(),
        size: source.size,
        mtime: source.mtime,
        at,
    }
}
