use tracing::debug;

use crate::device_state::{LocalEntry, LocalEntryState};
use crate::fetch::descent_error::DescentError;
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::standing::Standing;
use crate::fetch::surfaced::Surfaced;
use crate::fetch::target::Target;
use crate::index::Index;

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
/// The second question is asked *inside* the mapped root. The look descends the
/// Entry Path's components from the root one at a time, so a symbolic link on
/// the way down is refused rather than answered through: what stands past such a
/// link is not this device's mapped folder, and reporting on it — or worse,
/// deciding from it that the place is free — would be vouching for a file
/// outside the root (spec: EP-4, EP-11). That refusal is a finding like the
/// others and not a failure of the run: one folder of one mapped root having the
/// wrong shape says nothing about the next Entry.
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
        let standing = match target.place.look().await {
            Ok(standing) => standing,
            // A folder on the way to the place is not a folder of the mapped
            // root. Nothing can be placed here and everything else in the run
            // still can, so the Entry is reported and the next one is asked
            // about (spec: EP-4, EP-11).
            Err(DescentError::Blocked { path: component }) => {
                selection.surfaced.push(Surfaced::UnreachablePlace {
                    path: target.location.entry.path,
                    component,
                });
                continue;
            }
            // The disk itself would not answer, which is not a verdict about
            // this path and would not be one about the next.
            Err(refused) => return Err(FetchError::from_descent(refused, target.path())),
        };

        match (local, standing) {
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
            (Some(local), Some(standing)) if materialized(&local, &standing) => {
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

/// Whether what is on disk is still the materialization this device recorded
/// (spec: EP-10).
///
/// Two readings of one path, and they are not the same word: `standing` is what
/// the look found there now, and `local.observation` is what this device wrote
/// down when it last put a file there.
fn materialized(local: &LocalEntry, standing: &Standing) -> bool {
    standing.is_file
        && local.observation.size == standing.size
        && local.observation.mtime == standing.mtime
}
