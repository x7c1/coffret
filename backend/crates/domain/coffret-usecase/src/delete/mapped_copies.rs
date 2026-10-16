use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use coffret_model::{EntryPath, Redacted};
use tracing::{info, warn};

use crate::departure::{mapped_source, move_departed_to_trash, unedited, Departed, EmptiedFolders};
use crate::descent_error::DescentError;
use crate::device_state::{LocalEntry, LocalEntryState, Mapping};
use crate::fetch::reaching;
use crate::index::Index;
use crate::index_error::IndexResult;
use crate::local_error::LocalError;
use crate::local_scan::{assess_root, RootState};
use crate::local_trash::LocalTrash;
use crate::mapped_roots::MappedRoots;
use crate::sync::Surfaced;
use crate::unavailable_root::UnavailableRoot;

/// What a deletion reaches this device's own copies of the Entries it deleted
/// through (spec: EP-15).
///
/// The two capabilities a sync moves a departed file with, and for the same
/// reason each is one: the folders are read to find the file and to tell an
/// unedited copy from an edited one, and the trash is asked to take it.
#[derive(Clone, Copy)]
pub struct MappedCopies<'a> {
    /// The folders this device maps into the Library.
    pub roots: &'a dyn MappedRoots,
    /// The desktop's own trash.
    pub trash: &'a dyn LocalTrash,
}

/// What became of this device's own copies of the deleted Entries once the
/// deletion's commit had landed (spec: EP-15).
#[derive(Debug, Default)]
pub(super) struct MappedCopiesOutcome {
    /// Each copy moved to the trash, kept because it was edited, or whose move
    /// was refused, in Entry Path order.
    pub(super) surfaced: Vec<Surfaced>,
    /// The mappings whose roots this device could not vouch for, whose copies
    /// were left untouched for the next sync (spec: EP-12).
    pub(super) unavailable: Vec<UnavailableRoot>,
}

/// What one mapped root came to when the run asked about it, asked once for
/// all the copies under it.
enum Verdict {
    /// There to be read from: its copies are looked at.
    Available,
    /// Not there to be read from, and reported once: its copies stay.
    Unavailable,
    /// The question itself failed, and every copy under it is reported with
    /// what failed it.
    Unasked(Arc<DescentError>),
}

/// Applies EP-15 to this device's own copies of `deleted`, the Entries the
/// commit has just taken out of the Library — what the next sync would
/// otherwise do with them.
///
/// The decision is the sync's, through the same shared steps
/// ([`departure`](crate::departure)): a copy whose row says present and that
/// still holds what this device last made it match goes to the desktop's
/// trash and its row is forgotten, and the folders those moves emptied are
/// removed; an edited one is kept and reported; one whose file is gone from
/// disk too has its row forgotten and nothing said of it. The same root checks
/// guard it as a sync's: a root that is missing, or empty on a filesystem the
/// mapping does not record, is reported and nothing under it is touched or
/// forgotten (spec: EP-12), and every move holds the root to the identity its
/// mapping expects before it writes (spec: EP-13).
///
/// Nothing here fails the run. The commit has already landed, and a deletion
/// reported as failed after its batch committed would tell the person their
/// files are still in the Library. So a refusal from the disk or the trash is
/// reported with the copy it was about, and a catalog that would not answer is
/// logged, and either way what was left is the next sync's to finish: it finds
/// the same rows and asks the same questions.
pub(super) async fn move_mapped_copies_to_trash(
    index: &dyn Index,
    copies: MappedCopies<'_>,
    deleted: &[EntryPath],
) -> MappedCopiesOutcome {
    let MappedCopies { roots, trash } = copies;
    let mut found = MappedCopiesOutcome::default();
    if deleted.is_empty() {
        return found;
    }
    let mappings = match index.mappings().await {
        Ok(mappings) => mappings,
        Err(error) => {
            warn!(
                reason = %error.redacted(),
                "the mappings could not be read after a deletion; this device's copies of the \
                 deleted Entries are left for the next sync",
            );
            return found;
        }
    };

    // Keyed by the mapping's prefix, which names one mapping of a device.
    let mut verdicts: BTreeMap<Option<EntryPath>, Verdict> = BTreeMap::new();
    let mut emptied = EmptiedFolders::default();
    for path in deleted {
        let Some(mapping) = reaching(&mappings, path) else {
            // No mapping reaches the path, so no file of this device's stands
            // for it (spec: EP-9).
            continue;
        };
        let local = match departed_row(index, path).await {
            Ok(Some(local)) => local,
            Ok(None) => continue,
            Err(error) => {
                warn!(
                    reason = %error.redacted(),
                    "this device's row for a deleted Entry could not be read; its copy is left \
                     for the next sync",
                );
                continue;
            }
        };

        let verdict = match verdicts.get(&mapping.prefix) {
            Some(verdict) => verdict,
            None => {
                let verdict = assessed(roots, mapping, &mut found).await;
                verdicts.entry(mapping.prefix.clone()).or_insert(verdict)
            }
        };
        match verdict {
            Verdict::Available => {}
            Verdict::Unavailable => continue,
            Verdict::Unasked(cause) => {
                found.surfaced.push(Surfaced::MoveToTrashRefused {
                    path: path.clone(),
                    cause: cause.clone(),
                });
                continue;
            }
        }

        let source = match mapped_source(roots, mapping, path).await {
            Ok(Some(source)) => source,
            Ok(None) => {
                // Gone from the disk as well as from the Library: nothing is
                // left for a finding to be about, and the row goes, as the
                // sync's scan would forget it (spec: EP-15).
                if let Err(error) = index.forget_local_entry(path).await {
                    warn!(
                        reason = %error.redacted(),
                        "the row of a deleted Entry whose file is gone could not be forgotten; \
                         the next sync forgets it",
                    );
                }
                continue;
            }
            Err(refused) => {
                found
                    .surfaced
                    .push(refused_with(path, DescentError::Io(refused)));
                continue;
            }
        };

        match unedited(roots, &source, &local).await {
            Ok(true) => {}
            Ok(false) => {
                found
                    .surfaced
                    .push(Surfaced::KeptEdited { path: path.clone() });
                continue;
            }
            Err(refused) => {
                found
                    .surfaced
                    .push(refused_with(path, DescentError::Io(refused)));
                continue;
            }
        }

        let root = source.root.clone();
        let relative = source.relative.clone();
        let expected = mapping.expected_root_id;
        let surfaced =
            match move_departed_to_trash(index, trash, Departed { source, expected }).await {
                Ok(surfaced) => surfaced,
                Err(error) => {
                    // The file did go; only its row stayed, and the next sync,
                    // finding a row and no file, forgets it.
                    warn!(
                        reason = %error.redacted(),
                        "a deleted Entry's local file went to the trash and its row could not be \
                         forgotten; the next sync forgets it",
                    );
                    Surfaced::MovedToTrash { path: path.clone() }
                }
            };
        if matches!(surfaced, Surfaced::MovedToTrash { .. }) {
            emptied.moved(&root, expected, &relative);
        }
        found.surfaced.push(surfaced);
    }
    // Once every move is made, so a folder several copies stood in is asked
    // about after the last of them left it.
    emptied.remove(trash).await;

    info!(
        moved_to_trash = found
            .surfaced
            .iter()
            .filter(|one| matches!(one, Surfaced::MovedToTrash { .. }))
            .count(),
        kept_edited = found
            .surfaced
            .iter()
            .filter(|one| matches!(one, Surfaced::KeptEdited { .. }))
            .count(),
        refused = found
            .surfaced
            .iter()
            .filter(|one| matches!(one, Surfaced::MoveToTrashRefused { .. }))
            .count(),
        // A count and nothing else: a mapping names a prefix and a local root,
        // and neither may reach a diagnostic event.
        unavailable = found.unavailable.len(),
        "a deletion dealt with this device's copies of what it deleted",
    );
    found
}

/// This device's present row for a deleted Entry, where the path is departed:
/// the device put a file there and no current Entry stands there now
/// (spec: EP-15).
///
/// `None` for a path whose row is absent or gone, which holds nothing of this
/// device's, and for one another device has since added an Entry at again,
/// which the existing rules decide (spec: EP-10).
async fn departed_row(index: &dyn Index, path: &EntryPath) -> IndexResult<Option<LocalEntry>> {
    let Some(local) = index
        .local_entry_at(path)
        .await?
        .filter(|local| local.state == LocalEntryState::Present)
    else {
        return Ok(None);
    };
    if index.entry_at(path).await?.is_some() {
        return Ok(None);
    }
    Ok(Some(local))
}

/// Asks one mapped root whether it is there to be read from (spec: EP-12),
/// recording it in `found` where it is not.
///
/// Asked and not stamped: a root whose filesystem moved while it holds files
/// is one to look in, and recording what it stands on now is the sync's to do.
async fn assessed(
    roots: &dyn MappedRoots,
    mapping: &Mapping,
    found: &mut MappedCopiesOutcome,
) -> Verdict {
    match assess_root(roots, mapping).await {
        Ok(RootState::Available | RootState::Stamp(_)) => Verdict::Available,
        Ok(RootState::Unavailable(reason)) => {
            found.unavailable.push(UnavailableRoot {
                prefix: mapping.prefix.clone(),
                local_root: mapping.local_root.clone(),
                reason,
            });
            Verdict::Unavailable
        }
        Err(error) => {
            let cause = descent(error, &mapping.local_root);
            warn!(
                reason = %cause.redacted(),
                "a mapped root could not be asked about after a deletion; this device's copies \
                 under it are left for the next sync",
            );
            Verdict::Unasked(Arc::new(cause))
        }
    }
}

/// A copy whose move was not made, with what stopped it.
fn refused_with(path: &EntryPath, cause: DescentError) -> Surfaced {
    // What refused and of what kind, and nothing that names the file: the path
    // stays in the finding, for whoever asked for the run.
    warn!(
        reason = %cause.redacted(),
        "a deleted Entry's local file could not be looked at; it is left in place for the next \
         sync",
    );
    Surfaced::MoveToTrashRefused {
        path: path.clone(),
        cause: Arc::new(cause),
    }
}

/// What reading a mapped folder failed with, in the words a refused move is
/// reported in.
///
/// A refusal of the disk is what it is. A name in the root that folds to the
/// management area without being it, which is all the root check can meet
/// besides (spec: EP-14), is a name that stands in the way of the root being
/// read as anything, and is reported as blocking it at that name — the
/// sync stops on the same name and says so.
fn descent(error: LocalError, root: &Path) -> DescentError {
    match error {
        LocalError::Io(refused) => DescentError::Io(refused),
        LocalError::FoldedReservedName { path } | LocalError::UnrepresentableName { path } => {
            DescentError::Blocked { stopped_at: path }
        }
        // Two files claiming one Entry Path is what a whole walk can meet, and
        // the root check does not walk; answered all the same, at the root the
        // question was about, rather than assumed away.
        LocalError::PathCollision { .. } => DescentError::Blocked {
            stopped_at: root.to_path_buf(),
        },
    }
}
