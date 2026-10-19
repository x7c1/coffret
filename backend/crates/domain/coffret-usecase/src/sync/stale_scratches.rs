use coffret_model::Redacted;
use tracing::{debug, warn};

use crate::local_scan::StaleScratch;
use crate::local_trash::LocalTrash;

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
pub(super) async fn remove(trash: &dyn LocalTrash, stale: Vec<StaleScratch>) {
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
}
