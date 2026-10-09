use std::collections::BTreeMap;

use coffret_model::{ContainerId, ContainerKind, ContentHash};

use crate::device_state::{
    DeviceTime, LocalEntry, LocalEntryState, LocalObservation, RootMarkerId,
};
use crate::index::Index;
use crate::local_scan::SourceFile;
use crate::mapped_roots::MappedRoots;
use crate::sync::candidate::Candidate;
use crate::sync::departed::Departed;
use crate::sync::surfaced::Surfaced;
use crate::sync::survey::Survey;
use crate::sync::sync_error::SyncResult;

/// Decides what one local file means for the Library.
///
/// `expected` is the identity the mapping the file was found under expects of
/// its root, which a file this step sends to the trash carries to the move
/// (spec: EP-13, EP-15).
pub(super) async fn examine(
    index: &dyn Index,
    roots: &dyn MappedRoots,
    kinds: &BTreeMap<ContainerId, ContainerKind>,
    now: DeviceTime,
    source: &SourceFile,
    expected: Option<RootMarkerId>,
    survey: &mut Survey,
) -> SyncResult<()> {
    let Some(location) = index.entry_at(&source.path).await? else {
        return without_entry(index, roots, source, expected, survey).await;
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

    if hashed(roots, source).await? == location.entry.hash {
        // Touched and not changed: the content the Library holds is still the
        // content on disk, so only what this device last saw of the file moves.
        survey.unchanged += 1;
        survey
            .refreshed
            .push(observation(source, now, location.entry.hash));
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

/// Decides what a file standing where the Library holds no Entry is.
///
/// New, unless this device materialized the path and still records it present:
/// then the Entry it put there has left the Library since, and the file is
/// *departed* (spec: EP-15). An unedited one goes to the trash, which is the
/// run's to do once the scan is over; an edited one is kept where it is and
/// reported, and is never carried back in, because what it holds is the
/// person's change to a file the Library no longer has. A row that is already
/// absent is not this: the file standing there now is one the person put back,
/// and new is all this device can say of it (spec: EP-10).
async fn without_entry(
    index: &dyn Index,
    roots: &dyn MappedRoots,
    source: &SourceFile,
    expected: Option<RootMarkerId>,
    survey: &mut Survey,
) -> SyncResult<()> {
    let materialized = index
        .local_entry_at(&source.path)
        .await?
        .filter(|local| local.state == LocalEntryState::Present);
    let Some(local) = materialized else {
        survey.candidates.push(Candidate {
            source: source.clone(),
            replaces: None,
        });
        return Ok(());
    };
    if unedited(roots, source, &local).await? {
        survey.departed.push(Departed {
            source: source.clone(),
            expected,
        });
    } else {
        survey.surfaced.push(Surfaced::KeptEdited {
            path: source.path.clone(),
        });
    }
    Ok(())
}

/// Whether a departed file still holds what this device last made it match
/// (spec: EP-15).
///
/// The cheap comparison first, as everywhere a scan compares: a file whose
/// length and modification time are what this device recorded is not opened.
/// One whose observation moved is hashed and held against the hash recorded
/// beside the row, since the Entry's own left the catalog with the Entry. A row
/// that recorded no hash — one an older build wrote — leaves nothing to hold
/// the content against, and the file is kept rather than guessed about.
async fn unedited(
    roots: &dyn MappedRoots,
    source: &SourceFile,
    local: &LocalEntry,
) -> SyncResult<bool> {
    if local.observation.size == source.size && local.observation.mtime == source.mtime {
        return Ok(true);
    }
    let Some(recorded) = local.observation.hash else {
        return Ok(false);
    };
    Ok(hashed(roots, source).await? == recorded)
}

/// The content hash of a file's whole plaintext.
async fn hashed(roots: &dyn MappedRoots, source: &SourceFile) -> SyncResult<ContentHash> {
    let content = source.read(roots).await?;
    Ok(ContentHash::from_bytes(*blake3::hash(&content).as_bytes()))
}

/// What this device saw of a file, stamped with when it looked and with the
/// content it was found to hold.
fn observation(source: &SourceFile, at: DeviceTime, hash: ContentHash) -> LocalObservation {
    LocalObservation {
        path: source.path.clone(),
        size: source.size,
        mtime: source.mtime,
        at,
        hash: Some(hash),
    }
}
