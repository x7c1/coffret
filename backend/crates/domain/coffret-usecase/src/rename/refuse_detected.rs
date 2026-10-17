use std::collections::BTreeSet;

use crate::commit::PendingRename;
use crate::index::Index;
use crate::index_error::IndexResult;
use crate::rename::plan::{clash, refuse_outright};
use crate::rename::rename_refusal::RenameRefusal;
use crate::rename::renamed_path::RenamedPath;
use crate::rename::rules::Rules;

/// Holds the renames a sync detected to every rule a rename planned with
/// [`rename_entries`](crate::rename::rename_entries) is held to, and answers,
/// for each, the refusal it meets or `None` where it may be committed
/// (spec: EP-10, FM-15).
///
/// Each one names the Entry standing at its `from`, and its `path` is one the
/// catalog holds no Entry at: a file this device materialized is gone, and a
/// file with its content stands where none did. So the questions are the
/// plan's, asked of a set rather than of one source: the same path on both
/// sides and the reserved namespace (as [`refuse_outright`] asks), a
/// destination held by an Entry the set does not rename away, one that would
/// make a path both a file and a folder, and FM-15's rename-set check.
///
/// A refused rename is taken out of the set, and the others are asked again
/// with its Entry left where it stands, until none is refused: an Entry one of
/// them would have renamed away is then one the rest have to make room for.
/// Each surviving rename is held to the rename-set check on its own, so that a
/// refusal names the rename it belongs to; a set of them adds nothing the check
/// could refuse that one alone does not, since no two share an Entry or a
/// destination and none renames onto a path another leaves.
pub(crate) async fn refuse_detected(
    index: &dyn Index,
    detected: &[PendingRename],
) -> IndexResult<Vec<Option<RenameRefusal>>> {
    let mut refusals: Vec<Option<RenameRefusal>> = detected
        .iter()
        .map(|one| refuse_outright(&one.from, &one.path).err())
        .collect();
    let current = index.entries_under(None).await?;
    loop {
        let leaving: BTreeSet<&str> = detected
            .iter()
            .zip(&refusals)
            .filter(|(_, refusal)| refusal.is_none())
            .map(|(one, _)| one.from.as_str())
            .collect();
        let staying: BTreeSet<&str> = current
            .iter()
            .map(|location| location.path().as_str())
            .filter(|path| !leaving.contains(path))
            .collect();
        let mut refused_more = false;
        for (one, refusal) in detected.iter().zip(refusals.iter_mut()) {
            if refusal.is_none() {
                *refusal = clash(&staying, &one.path);
                refused_more |= refusal.is_some();
            }
        }
        if !refused_more {
            break;
        }
    }

    let rules = Rules::read(index, &current).await?;
    for (one, refusal) in detected.iter().zip(refusals.iter_mut()) {
        if refusal.is_none() {
            *refusal = rules.refusal(&[(
                one.entry,
                RenamedPath {
                    from: one.from.clone(),
                    to: one.path.clone(),
                },
            )])?;
        }
    }
    Ok(refusals)
}
