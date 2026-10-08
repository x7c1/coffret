use std::collections::{BTreeMap, BTreeSet};

use coffret_format::EntryPlan;
use coffret_model::{ContainerId, ContainerKind, ContentHash, EntryLocation};

use crate::device_state::{DeviceTime, LocalEntryState, LocalObservation};
use crate::freeze::freeze_error::FreezeResult;
use crate::freeze::not_frozen::NotFrozen;
use crate::freeze::selected::Selected;
use crate::freeze::survey::Survey;
use crate::index::Index;
use crate::local_scan::SourceFile;
use crate::mapped_roots::MappedRoots;

/// What one local file is to a freeze, as far as the catalog and the file's
/// stat can say without reading a byte of it.
///
/// This is the whole of the selection: [`examine`] acts on it, and a preview
/// counts it (spec: PK-1, PK-2, EP-10). What it does not decide is the one
/// question only a hash answers — whether a Pack-held file whose stat moved
/// has really changed — and that question is about what is surfaced, never
/// about what is selected.
#[derive(Debug, Clone)]
pub(super) enum Verdict {
    /// Not in the Library at all: an initial import, which a freeze builds a
    /// Pack from directly (spec: PK-7).
    New,
    /// Held by a one-file Container, which the Pack absorbs however the local
    /// file compares and whether or not the Container's key survives
    /// (spec: PK-1, PK-13).
    Absorbs(ContainerId),
    /// A current Entry this device never materialized, which is outside its
    /// scope whether or not a mapping covers it (spec: EP-10).
    NotMaterialized,
    /// Held by a Pack the committed Keyring records no key for (spec: KL-7,
    /// PK-11).
    KeyLostInPack(ContainerId),
    /// Held by a Pack, and the file still stands as this device last saw it
    /// (spec: PK-2).
    InPack,
    /// Held by a Pack, and the file's length or modification time moved since
    /// this device last saw it: a hash says whether its content did
    /// (spec: PK-14).
    InPackTouched(EntryLocation),
}

/// Decides what one local file is to this invocation, from the catalog and
/// the file's stat alone.
pub(super) async fn judge(
    index: &dyn Index,
    kinds: &BTreeMap<ContainerId, ContainerKind>,
    key_lost: &BTreeSet<ContainerId>,
    source: &SourceFile,
) -> FreezeResult<Verdict> {
    let Some(location) = index.entry_at(&source.path).await? else {
        return Ok(Verdict::New);
    };

    // Packing a file this device never put there would propose replacing an
    // Entry from a file this device never placed (spec: EP-10).
    let Some(local) = index.local_entry_at(&source.path).await? else {
        return Ok(Verdict::NotMaterialized);
    };
    let container_id = location.container_id;

    if kinds.get(&container_id) == Some(&ContainerKind::OneFile) {
        return Ok(Verdict::Absorbs(container_id));
    }

    // From here the Entry is held by a Pack, which a freeze never reads and
    // never rewrites (spec: PK-1, PK-2).
    if key_lost.contains(&container_id) {
        return Ok(Verdict::KeyLostInPack(container_id));
    }
    if local.state == LocalEntryState::Present
        && local.observation.size == source.size
        && local.observation.mtime == source.mtime
    {
        return Ok(Verdict::InPack);
    }
    Ok(Verdict::InPackTouched(location))
}

/// Decides what one local file means for this invocation, and acts on it.
///
/// Eight of them, and none is one this step could derive: the catalog and the
/// disk it reads through, the two things eligibility is decided against, the
/// clock the refreshed observations are stamped with, the file itself, the
/// buffer a hash is taken over, and what the answers are collected into.
#[allow(clippy::too_many_arguments)]
pub(super) async fn examine(
    index: &dyn Index,
    roots: &dyn MappedRoots,
    kinds: &BTreeMap<ContainerId, ContainerKind>,
    key_lost: &BTreeSet<ContainerId>,
    now: DeviceTime,
    source: &SourceFile,
    buffer: &mut [u8],
    survey: &mut Survey,
) -> FreezeResult<()> {
    match judge(index, kinds, key_lost, source).await? {
        Verdict::New => {
            // A freeze does not upload a one-file Container first and absorb
            // it afterwards (spec: PK-7).
            let plan = plan(source, hashed(source, roots, buffer).await?);
            survey.selected.push(Selected {
                source: source.clone(),
                plan,
                absorbs: None,
            });
        }
        Verdict::Absorbs(container_id) => {
            // The replacement is built from the bytes on disk either way
            // (spec: PK-1, PK-13).
            let plan = plan(source, hashed(source, roots, buffer).await?);
            survey.selected.push(Selected {
                source: source.clone(),
                plan,
                absorbs: Some(container_id),
            });
        }
        Verdict::NotMaterialized => {}
        // What is left to decide for a Pack-held Entry is whether the file
        // needs an update, because that is what may not be passed over
        // quietly (spec: PK-14).
        Verdict::KeyLostInPack(container_id) => {
            survey.surfaced.push(NotFrozen::KeyLostInPack {
                path: source.path.clone(),
                container_id,
            });
        }
        Verdict::InPack => survey.packed_already += 1,
        Verdict::InPackTouched(location) => {
            if hashed(source, roots, buffer).await?.0 == location.entry.hash {
                // Touched and not changed: the content the Library holds is
                // still the content on disk, so only what this device last saw
                // of the file moves.
                survey.packed_already += 1;
                survey.refreshed.push(LocalObservation {
                    path: source.path.clone(),
                    size: source.size,
                    mtime: source.mtime,
                    at: now,
                });
            } else {
                survey.surfaced.push(NotFrozen::ModifiedInPack {
                    path: source.path.clone(),
                    container_id: location.container_id,
                });
            }
        }
    }
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
async fn hashed(
    source: &SourceFile,
    roots: &dyn MappedRoots,
    buffer: &mut [u8],
) -> FreezeResult<(ContentHash, u64)> {
    let mut reader = source.reader(roots).await?;
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
