use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, ContainerKind, EntryPath};
use tracing::debug;

use crate::device_state::{DeviceTime, Mapping};
use crate::freeze::freeze_error::FreezeResult;
use crate::freeze::survey::Survey;
use crate::index::Index;
use crate::local_scan::{unavailable_roots, walk_mappings, RootState, Walked};
use crate::spool_file::WRITE_CHUNK;

mod examine;
use examine::examine;

/// Walks the folder and decides what this invocation can pack.
///
/// The eligibility rule is PK-1's, and it is about the Container kind rather
/// than the Entry count: a file not yet in the Library, or one whose current
/// Entry is held by a one-file Container. The second half holds however the
/// local file compares — a modification and a lost key are both eligible here,
/// because either way the replacement is built from the local bytes (spec:
/// PK-13). An Entry a Pack already holds is never eligible, and existing Packs
/// are neither read nor listed for removal (spec: PK-1, PK-2).
///
/// Scope is EP-10's, exactly as the sync reads it: a path with a current Entry
/// and no local materialization row is one this device never put on disk, and a
/// mapping covering it does not change that. Such a path is left alone rather
/// than surfaced — it is outside this device's scope, not a file it is failing
/// to back up.
///
/// A mapped root the device cannot vouch for is bounded out the same way the
/// sync bounds it (spec: EP-12): nothing under it is walked, so it contributes no
/// candidate, and it is reported in [`FreezeOutcome::unavailable`] rather than
/// being passed over. A freeze infers no deletion, so the only harm such a root
/// can do it is silence — and a run that packed nothing because a disk is
/// unplugged looks exactly like the second run over an already-packed folder,
/// which is why the mapping is reported. A root that holds files on a filesystem
/// the mapping does not record is available and re-stamped, which is this step's
/// own write through the port rather than the walk's.
///
/// The prefix bounds the same question a second way. A freeze selects the
/// eligible files under the folder one invocation names (spec: PK-17), so a file
/// outside it is not a candidate this scan passed over quietly: PK-14 governs
/// what a scan may keep silent about among the files it considered, and a run
/// over that other folder — or over the Library root — is what considers the
/// rest.
///
/// The expensive comparison is paid only where it decides something. Every
/// selected file is hashed, because a Pack's entry table has to be written
/// before its content and the hash is part of that table. A Pack-resident Entry
/// is hashed only when the cheap comparison says the file may have moved, and a
/// file that turns out unchanged costs a read and refreshes what this device
/// last saw of it rather than being read again next time.
///
/// [`FreezeOutcome::unavailable`]: crate::freeze::FreezeOutcome::unavailable
pub(super) async fn scan(
    index: &dyn Index,
    prefix: Option<&EntryPath>,
    key_lost: &BTreeSet<ContainerId>,
    now: DeviceTime,
) -> FreezeResult<Survey> {
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

    // The kind is what decides eligibility, and the port answers kinds a prefix
    // at a time. One walk under the run's own prefix answers every lookup below,
    // however many mappings overlap it and whichever Containers their Entries
    // turn out to share (spec: PK-8).
    let kinds: BTreeMap<ContainerId, ContainerKind> = index
        .containers_under(prefix)
        .await?
        .into_iter()
        .map(|container| (container.id, container.kind))
        .collect();

    let mut survey = Survey {
        selected: Vec::new(),
        packed_already: 0,
        surfaced: Vec::new(),
        refreshed: Vec::new(),
        unavailable: unavailable_roots(&roots),
    };
    // `found` is keyed by Entry Path, so this walk is already the order
    // segmentation needs (spec: EP-3, PK-3).
    let mut considered = 0usize;
    let mut buffer = vec![0u8; WRITE_CHUNK];
    for source in found.values() {
        if prefix.is_some_and(|prefix| !source.path.is_under(prefix)) {
            continue;
        }
        considered += 1;
        examine(
            index,
            &kinds,
            key_lost,
            now,
            source,
            &mut buffer,
            &mut survey,
        )
        .await?;
    }

    // Counts only: a prefix is an Entry Path component and a local root is a
    // local path, and neither may reach a log line.
    debug!(
        mappings = mappings.len(),
        files = considered,
        selected = survey.selected.len(),
        packed_already = survey.packed_already,
        surfaced = survey.surfaced.len(),
        unavailable = survey.unavailable.len(),
        "scanned a folder for freezing",
    );
    Ok(survey)
}
