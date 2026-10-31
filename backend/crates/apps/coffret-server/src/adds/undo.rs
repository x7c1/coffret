use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::Arc;

use coffret_device::{KeptAside, OpenLibrary, Redacted};
use tracing::{debug, info, warn};

use crate::activity_log::{ActivityLog, Ended};

use super::{Add, Replaced, StoppedAdd, Written};

/// What every event of an undo names its operation as.
const OPERATION: &str = "stop";

/// Undoes an add a person stopped, on this device (spec: LA-36).
///
/// Run by the flow that carries the add, under its turn at this device's
/// pending work and before that flow scans again — or, for a Pack drop still
/// receiving, by its own request, whose live reservation keeps every run off
/// its files (spec: PK-17) — so no run carries in a file this is removing.
///
/// Every file the add wrote is taken away, and nothing else: one written where
/// nothing stood is removed, and one written over a file of the person's is
/// replaced by that file, which the add kept aside before writing over it —
/// never one replaced by another since it landed, and never the file an
/// import was made from, which is not in the folder at all. A file a run
/// committed meanwhile is the Library's, and stays; what was kept aside for it
/// is let go with the add. An add that wrote over a file it could not keep
/// aside is never undone, since its stop is refused
/// ([`Adds::ask_stop`](super::Adds::ask_stop)). Then the folders the add made
/// on the way are removed where it left them empty, and the reservation its
/// drop made for its Pack is released; one it took over from a drop cut off
/// before it is kept, for the files that drop received (spec: PK-17). What the
/// add sent to Storage without committing is left to the cleanup of orphaned
/// objects (spec: OC-2).
pub(crate) async fn undo(library: &OpenLibrary, add: Add) -> StoppedAdd {
    let mut undone = StoppedAdd {
        folder: add.folder.clone(),
        files: add.files.len(),
        removed: 0,
        committed: 0,
        left: 0,
    };
    for written in add.files.iter().rev() {
        match committed(library, written).await {
            Ok(true) => undone.committed += 1,
            Ok(false) if take_away(written) => undone.removed += 1,
            Ok(false) => undone.left += 1,
            Err(refused) => {
                warn!(
                    operation = OPERATION,
                    reason = %refused.redacted(),
                    "whether a file of a stopped add was committed could not be read; it is \
                     left in the folder",
                );
                undone.left += 1;
            }
        }
    }
    let mut made = add.made.clone();
    made.sort_by_key(|folder| std::cmp::Reverse(folder.components().count()));
    made.dedup();
    for folder in &made {
        match std::fs::remove_dir(folder) {
            Ok(()) => {}
            // Not empty, or gone already: either way the add has nothing left
            // to take away there.
            Err(cause) => debug!(
                operation = OPERATION,
                reason = %cause.kind(),
                "a folder a stopped add made was left where it stands",
            ),
        }
    }
    if let Some((reservation, false)) = add.reservation {
        if let Err(refused) = library.release_reservation(reservation).await {
            warn!(
                operation = OPERATION,
                reservation = %reservation,
                reason = %refused.redacted(),
                "the reservation of a stopped add could not be released",
            );
        }
    }
    // Counts only: the folder and the files are the person's own names
    // (spec: EL-1).
    info!(
        operation = OPERATION,
        files = undone.files,
        removed = undone.removed,
        committed = undone.committed,
        left = undone.left,
        "an add was stopped and undone on this device",
    );
    undone
}

/// Puts what an undo came to in the log, unless a run had committed every
/// file the add wrote: then the stop met a commit, and what answers it is
/// that run's own entry (spec: LA-36).
pub(crate) fn log_undone(log: &ActivityLog, undone: StoppedAdd) {
    if undone.files > 0 && undone.committed == undone.files {
        return;
    }
    log.record(Ended::Add(undone));
}

/// Whether a run committed the file since the add wrote it: the Library holds
/// an Entry at its path other than the one it held before the add — a new one
/// where none stood, or the one a replacement committed in place of the Entry
/// that stood there. Whether this device had that earlier Entry's file says
/// nothing about it either way.
async fn committed(library: &OpenLibrary, written: &Written) -> coffret_device::Result<bool> {
    let now = library.entry_at(&written.path).await?;
    Ok(now.is_some() && now != written.entry_before)
}

/// Takes away the file the add left, where it is still that file — removing
/// it, or putting back the file it replaced — and says whether nothing of it
/// is left.
fn take_away(written: &Written) -> bool {
    let kept = match &written.replaced {
        Replaced::Nothing => None,
        Replaced::Kept(kept) => Some(kept),
        // Never reached through a stop, which such an add refuses; left where
        // it is, since removing it would take the person's path with it.
        Replaced::Unkept => return false,
    };
    match still_ours(&written.local, written.identity) {
        Ok(true) => match kept {
            Some(kept) => put_back(kept),
            None => match std::fs::remove_file(&written.local) {
                Ok(()) => true,
                Err(cause) if cause.kind() == io::ErrorKind::NotFound => true,
                Err(cause) => {
                    warn!(
                        operation = OPERATION,
                        reason = %cause.kind(),
                        "a file of a stopped add could not be removed",
                    );
                    false
                }
            },
        },
        Ok(false) => false,
        // The add's file is gone. Where the person's file is still kept
        // aside, it goes back to its name all the same, as the one thing the
        // undo owes the person there.
        Err(cause) if cause.kind() == io::ErrorKind::NotFound => kept.is_none_or(put_back),
        Err(cause) => {
            warn!(
                operation = OPERATION,
                reason = %cause.kind(),
                "a file of a stopped add could not be looked at, so it is left",
            );
            false
        }
    }
}

/// Puts back the file an add replaced, and says whether it went back; a
/// refusal is reported rather than passed over, whichever branch asked.
fn put_back(kept: &Arc<KeptAside>) -> bool {
    match kept.put_back() {
        Ok(()) => true,
        Err(refused) => {
            warn!(
                operation = OPERATION,
                reason = %refused.redacted(),
                "the file a stopped add replaced could not be put back",
            );
            false
        }
    }
}

/// Whether the file at `local` is the one the add wrote: a regular file, not
/// a link, with the identity it was left with.
fn still_ours(local: &Path, identity: Option<(u64, u64)>) -> io::Result<bool> {
    let found = std::fs::symlink_metadata(local)?;
    Ok(found.file_type().is_file() && identity == Some((found.dev(), found.ino())))
}
