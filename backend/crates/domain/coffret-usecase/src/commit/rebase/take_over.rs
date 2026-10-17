use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerAddition, EntryPath};

use crate::commit::commit_error::{CommitError, CommitResult};
use crate::commit::entry_ref::EntryRef;
use crate::commit::prepared_batch::PreparedBatch;

/// Gives each of an addition's Entries that takes the place of another the
/// current path the intervening renames left that other Entry at, and answers
/// with the paths the Entries so given one were prepared under.
///
/// Only the current path changes. The captured one is what the new Container's
/// meta section records, and stays (spec: FM-9, FM-15).
pub(super) fn take_over(
    batch: &mut PreparedBatch,
    renamed: &BTreeMap<EntryRef, &EntryPath>,
) -> CommitResult<BTreeSet<EntryPath>> {
    let mut taken_over = BTreeSet::new();
    if renamed.is_empty() {
        return Ok(taken_over);
    }
    for prepared in &mut batch.additions {
        let followed: Vec<(usize, &EntryPath)> = prepared
            .supersedes
            .iter()
            .filter_map(|(number, superseded)| {
                renamed
                    .get(superseded)
                    .map(|to| (number.get() as usize, *to))
            })
            .collect();
        if followed.is_empty() {
            continue;
        }
        let (container, mut entries) = prepared.addition.clone().into_parts();
        for (position, to) in followed {
            if let Some(entry) = entries.get_mut(position) {
                if entry.path != *to {
                    taken_over.insert(entry.path.clone());
                    entry.path = to.clone();
                }
            }
        }
        // Two of the addition's Entries cannot end at one path: one that was
        // not given that path is a conflict `check_contested` refused, and two
        // that were would be two Entries the head holds at one path.
        prepared.addition = ContainerAddition::new(container, entries)
            .map_err(|cause| CommitError::UnwritableControlValue { cause })?;
    }
    Ok(taken_over)
}
