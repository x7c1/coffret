use coffret_model::Redacted;
use tracing::{debug, warn};

use crate::local_scan::{walk_mappings, StaleScratch, NONE_PLACED};
use crate::local_trash::LocalTrash;
use crate::mapped_roots::MappedRoots;
use crate::sync::sync_error::SyncResult;
use crate::Index;

/// Removes every stale scratch under every mapped folder this device can
/// vouch for, and answers how many it removed (spec: OC-8).
///
/// What a server asks as it opens the Library, before it serves anything. The
/// file an add keeps aside before writing over another (spec: LA-36) is held
/// only while that add lasts, and an add lasts no longer than the server that
/// carried it (spec: LA-12): one a server that died left behind is nobody's.
/// A sync removes it where its walk meets it, but the sync a drop arms walks
/// only the folders that drop wrote into (spec: EP-10), so one left anywhere
/// else could stay until a full sync. Removed, never put back: what stands at
/// the name now is what the person will see, and the kept file was the
/// earlier one.
///
/// Every stale scratch the walk meets goes, an interrupted fetch's or upload's
/// as well as a kept file: they are one shape, and one rule removes them. A
/// scratch another process's writer still holds is left, as a sync leaves it
/// (see [`LocalTrash::remove_scratch`]), and a root that is not there is not
/// walked (spec: EP-12).
///
/// # Errors
///
/// The refusals a sync's walk answers in: the mappings could not be read, or
/// the walk could not go on — a folder that cannot be read, a name the
/// Library cannot hold. Nothing has been removed past where the walk stopped,
/// and the caller is expected to go on without it: the next sync meets what
/// is left.
pub async fn remove_stale_scratches(
    index: &dyn Index,
    roots: &dyn MappedRoots,
    trash: &dyn LocalTrash,
) -> SyncResult<usize> {
    let mappings = index.mappings().await?;
    let walked = walk_mappings(roots, &mappings, &NONE_PLACED).await?;
    Ok(remove(trash, walked.stale).await)
}

/// Removes every scratch a local writer that is gone left under a root the
/// scan read (spec: OC-8, EP-11).
///
/// What a writer killed before its rename or its removal leaves is a file under
/// coffret's own scratch prefix that the scan steps over and nothing else ever
/// names, so without this it would stay in the person's folder for good. Only
/// the writers' own shape that no writer in this process holds reaches here
/// ([`scratch::is_stale`](crate::scratch::is_stale)), and the gateway leaves one
/// another process's writer still holds (see
/// [`LocalTrash::remove_scratch`]).
///
/// A refusal is logged and goes no further, the way a folder the discards
/// emptied is: nothing in the run depends on it, the scan already passed over
/// the file, and the next run meets it again and retries, which is safe
/// because the removal is idempotent (spec: OC-8).
///
/// Answers how many it removed.
pub(super) async fn remove(trash: &dyn LocalTrash, stale: Vec<StaleScratch>) -> usize {
    let mut removed = 0;
    for scratch in &stale {
        match trash
            .remove_scratch(&scratch.root, Some(&scratch.expected), &scratch.relative)
            .await
        {
            Ok(true) => removed += 1,
            Ok(false) => {}
            // What refused and of what kind, and nothing that names the file,
            // for the reason a refused discard names none (spec: EL-1).
            Err(refused) => warn!(
                reason = %refused.redacted(),
                "a scratch an interrupted write left could not be removed; the next sync retries",
            ),
        }
    }
    if !stale.is_empty() {
        debug!(
            stale = stale.len(),
            removed, "removed the scratches interrupted writes left in the mapped folders",
        );
    }
    removed
}
