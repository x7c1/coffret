use std::collections::{BTreeMap, BTreeSet};

use coffret_format::EntryPlan;
use coffret_model::{ContainerId, ContainerKind, ContentHash};

use crate::device_state::{DeviceTime, LocalEntryState, LocalObservation};
use crate::freeze::freeze_error::FreezeResult;
use crate::freeze::not_frozen::NotFrozen;
use crate::freeze::selected::Selected;
use crate::freeze::survey::Survey;
use crate::index::Index;
use crate::local_scan::SourceFile;

/// Decides what one local file means for this invocation.
pub(super) async fn examine(
    index: &dyn Index,
    kinds: &BTreeMap<ContainerId, ContainerKind>,
    key_lost: &BTreeSet<ContainerId>,
    now: DeviceTime,
    source: &SourceFile,
    buffer: &mut [u8],
    survey: &mut Survey,
) -> FreezeResult<()> {
    let Some(location) = index.entry_at(&source.path).await? else {
        // Not in the Library at all: an initial import, which a freeze builds a
        // Pack from directly rather than by uploading a one-file Container first
        // (spec: PK-7).
        let plan = plan(source, hashed(source, buffer).await?);
        survey.selected.push(Selected {
            source: source.clone(),
            plan,
            absorbs: None,
        });
        return Ok(());
    };

    // A current Entry this device never materialized is outside its scope,
    // whether or not a mapping covers it (spec: EP-10). Packing it would propose
    // replacing an Entry from a file this device never put there.
    let Some(local) = index.local_entry_at(&source.path).await? else {
        return Ok(());
    };
    let container_id = location.container_id;

    if kinds.get(&container_id) == Some(&ContainerKind::OneFile) {
        // Eligible however the local file compares, and whether or not the
        // Container's key survives: the replacement is built from the bytes on
        // disk either way (spec: PK-1, PK-13).
        let plan = plan(source, hashed(source, buffer).await?);
        survey.selected.push(Selected {
            source: source.clone(),
            plan,
            absorbs: Some(container_id),
        });
        return Ok(());
    }

    // From here the Entry is held by a Pack, which a freeze never reads and
    // never rewrites (spec: PK-1, PK-2). What is left to decide is whether the
    // file needs an update, because that is what may not be passed over quietly
    // (spec: PK-14).
    if key_lost.contains(&container_id) {
        survey.surfaced.push(NotFrozen::KeyLostInPack {
            path: source.path.clone(),
            container_id,
        });
        return Ok(());
    }
    if local.state == LocalEntryState::Present
        && local.observation.size == source.size
        && local.observation.mtime == source.mtime
    {
        survey.packed_already += 1;
        return Ok(());
    }
    if hashed(source, buffer).await?.0 == location.entry.hash {
        // Touched and not changed: the content the Library holds is still the
        // content on disk, so only what this device last saw of the file moves.
        survey.packed_already += 1;
        survey.refreshed.push(LocalObservation {
            path: source.path.clone(),
            size: source.size,
            mtime: source.mtime,
            at: now,
        });
        return Ok(());
    }
    survey.surfaced.push(NotFrozen::ModifiedInPack {
        path: source.path.clone(),
        container_id,
    });
    Ok(())
}

/// The BLAKE3-256 of a local file's plaintext, and how long it turned out to
/// be, read a buffer at a time.
///
/// Read rather than held: the file goes past the hasher and is not kept, so
/// hashing a folder of several hundred gigabytes costs one buffer.
///
/// The length comes back because it is the read's answer and not the stat's: a
/// file that grew between the two would otherwise be planned at one length and
/// hashed at another, and the disagreement would only surface as a refused
/// encode much later.
async fn hashed(source: &SourceFile, buffer: &mut [u8]) -> FreezeResult<(ContentHash, u64)> {
    let mut reader = source.open().await?;
    let mut hasher = blake3::Hasher::new();
    let mut read = 0u64;
    loop {
        let filled = reader.read(buffer).await?;
        if filled == 0 {
            return Ok((ContentHash::from_bytes(*hasher.finalize().as_bytes()), read));
        }
        hasher.update(&buffer[..filled]);
        read += filled as u64;
    }
}

/// What the Pack's entry table will say about one selected file.
///
/// The birth time comes along where the scan read one (spec: FM-9).
///
/// No MIME: detection is not a freeze's work, exactly as it is not a sync's.
fn plan(source: &SourceFile, (hash, size): (ContentHash, u64)) -> EntryPlan {
    EntryPlan {
        btime: source.btime,
        ..EntryPlan::new(source.path.clone(), source.mtime, size, hash)
    }
}
