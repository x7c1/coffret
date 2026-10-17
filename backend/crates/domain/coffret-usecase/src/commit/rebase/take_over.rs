use std::collections::BTreeMap;

use coffret_model::{ContainerAddition, EntryPath};

use crate::commit::commit_error::{CommitError, CommitResult};
use crate::commit::prepared_batch::PreparedBatch;
use crate::entry_ref::EntryRef;

/// Gives each of an addition's Entries that takes the place of another the
/// current path the intervening renames left that other Entry at.
///
/// Only the current path changes. The captured one is what the new Container's
/// meta section records, and stays (spec: FM-9, FM-15). So does the local file
/// the Entry was drawn from, and the batch's record of it: it names the new
/// Entry, so the refresh records the file waiting to follow it there
/// (spec: EP-10).
pub(super) fn take_over(
    batch: &mut PreparedBatch,
    renamed: &BTreeMap<EntryRef, &EntryPath>,
) -> CommitResult<()> {
    if renamed.is_empty() {
        return Ok(());
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
                entry.path = to.clone();
            }
        }
        // Two of the addition's Entries cannot end at one path: one that was
        // not given that path is a conflict `check_contested` refused, and two
        // that were would be two Entries the head holds at one path.
        prepared.addition = ContainerAddition::new(container, entries)
            .map_err(|cause| CommitError::UnwritableControlValue { cause })?;
    }
    Ok(())
}
