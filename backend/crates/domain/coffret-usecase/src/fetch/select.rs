use std::io;
use std::path::Path;

use coffret_model::Mtime;
use tokio::fs;
use tracing::debug;

use crate::device_state::{LocalEntry, LocalEntryState};
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::surfaced::Surfaced;
use crate::fetch::target::Target;
use crate::index::Index;
use crate::local_mtime::mtime_of;
use crate::local_operation::LocalOperation;

/// What the run will fetch, and what it will only report.
pub(super) struct Selection {
    /// The Entries whose target paths the device can vouch for (spec: EP-11).
    pub(super) wanted: Vec<Target>,
    /// How many were already materialized here.
    pub(super) skipped: usize,
    /// Everything else, with the reason.
    pub(super) surfaced: Vec<Surfaced>,
}

/// Decides, for each Entry a mapping translated, whether a fetch may write
/// there.
///
/// Two questions per target and no more: what this device wrote down about the
/// path, and what is on disk at it now. Between them they pick out the two
/// states EP-11 admits — nothing there, or this device's own materialization
/// still matching what it recorded — and everything else is a finding.
///
/// The comparison against a materialization record is the cheap one, length and
/// modification time, and deliberately not a hash. A file whose stamp has moved
/// is a *local* change the sync flow owns: settling whether the content really
/// differs is that flow's job, done against the Entry it would then replace
/// (spec: EP-10). A fetch that hashed here would be answering the same question
/// twice and would still not be allowed to write.
pub(super) async fn select(index: &dyn Index, targets: Vec<Target>) -> FetchResult<Selection> {
    let mut selection = Selection {
        wanted: Vec::new(),
        skipped: 0,
        surfaced: Vec::new(),
    };

    for target in targets {
        let local = index.local_entry_at(target.path()).await?;
        let observed = observe(&target.local_path).await?;

        match (local, observed) {
            // Outside this device's scope and nothing standing in the way: the
            // one state a fetch may claim (spec: EP-10, EP-11).
            (None, None) => selection.wanted.push(target),
            // A file this device never placed. It may be an unsynced source
            // file, so it is reported and left exactly as it is.
            (None, Some(_)) => selection.surfaced.push(Surfaced::ForeignFile {
                path: target.location.entry.path,
            }),
            // A deletion this device witnessed. Putting the file back is an
            // explicit operation and never one inferred from the row
            // (spec: EP-10).
            (Some(local), _) if local.state == LocalEntryState::Absent => {
                selection.surfaced.push(Surfaced::WitnessedDeletion {
                    path: target.location.entry.path,
                })
            }
            // This device's own materialization, still as it left it: the file
            // *is* the Entry, so there is nothing to fetch.
            (Some(local), Some(observed)) if materialized(&local, &observed) => {
                selection.skipped += 1;
            }
            // This device placed the file and it is no longer what it wrote
            // down — changed, or gone. Either way a pending local change the
            // sync flow owns, and re-fetching would quietly undo it.
            (Some(_), _) => selection.surfaced.push(Surfaced::LocallyChanged {
                path: target.location.entry.path,
            }),
        }
    }

    debug!(
        wanted = selection.wanted.len(),
        skipped = selection.skipped,
        surfaced = selection.surfaced.len(),
        "selected what to fetch into the mapped folders",
    );
    Ok(selection)
}

/// What the filesystem says stands at a target path now.
///
/// `symlink_metadata` rather than `metadata`, for the reason a scan uses it: a
/// symbolic link is not the file it points at (spec: EP-8). A link, a directory,
/// or anything else that is not a regular file is still *something in the way*,
/// which is what the caller has to know.
struct Observed {
    size: u64,
    mtime: Mtime,
    is_file: bool,
}

/// Stats one target path, `None` meaning nothing is there.
async fn observe(local_path: &Path) -> FetchResult<Option<Observed>> {
    match fs::symlink_metadata(local_path).await {
        Ok(metadata) => Ok(Some(Observed {
            size: metadata.len(),
            mtime: mtime_of(&metadata),
            is_file: metadata.is_file(),
        })),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(cause) => Err(FetchError::Io {
            operation: LocalOperation::Stating,
            path: local_path.to_path_buf(),
            cause,
        }),
    }
}

/// Whether what is on disk is still the materialization this device recorded
/// (spec: EP-10).
fn materialized(local: &LocalEntry, observed: &Observed) -> bool {
    observed.is_file
        && local.observation.size == observed.size
        && local.observation.mtime == observed.mtime
}
