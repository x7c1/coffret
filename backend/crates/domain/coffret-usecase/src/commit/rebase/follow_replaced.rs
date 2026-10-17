use coffret_model::{EntryPath, JournalRecord};

use super::number_at;
use crate::commit::commit_error::{CommitError, CommitResult};
use crate::commit::pending_change::PendingChange;
use crate::commit::prepared_batch::PreparedBatch;
use crate::commit::rebase_rule::RebaseRule;
use crate::entry_ref::EntryRef;

/// Carries each rename of the batch through the intervening records: onto the
/// Entry a record that removed its Container added at the name the rename was
/// prepared against, or to a conflict where a record renamed its Entry or
/// removed it with nothing at that name in its place.
pub(super) fn follow_replaced(
    batch: &mut PreparedBatch,
    intervening: &[JournalRecord],
) -> CommitResult<()> {
    for rename in &mut batch.renames {
        for record in intervening {
            let conflict = |rule| CommitError::RebaseConflict {
                pending: PendingChange::Rename(rename.entry),
                record: record.generation(),
                rule,
                path: rename.from.clone(),
            };
            if record
                .renames()
                .iter()
                .any(|other| EntryRef::renamed_by(other) == rename.entry)
            {
                return Err(conflict(RebaseRule::RenamedEntryRenamed));
            }
            // The record's removals are sorted by Container ID (spec: FM-15).
            if record
                .removals()
                .binary_search(&rename.entry.container_id)
                .is_ok()
            {
                let Some(successor) = added_at(record, &rename.from) else {
                    return Err(conflict(RebaseRule::RenamedEntryReplaced));
                };
                rename.entry = successor;
            }
        }
    }
    Ok(())
}

/// The Entry `record` adds at `path`, if it adds one there.
fn added_at(record: &JournalRecord, path: &EntryPath) -> Option<EntryRef> {
    record.additions().iter().find_map(|addition| {
        let position = addition
            .entries()
            .iter()
            .position(|entry| entry.path == *path)?;
        Some(EntryRef::new(addition.container().id, number_at(position)))
    })
}
