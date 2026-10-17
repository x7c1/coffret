use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{EntryLocation, EntryPath, Generation, JournalRecord};

use super::number_at;
use crate::commit::commit_error::{CommitError, CommitResult};
use crate::commit::entry_ref::EntryRef;
use crate::commit::pending_change::PendingChange;
use crate::commit::prepared_batch::PreparedBatch;
use crate::commit::rebase_rule::RebaseRule;
use crate::index::Index;

/// Refuses an addition whose path an intervening rename gave another Entry,
/// and a rename whose destination an intervening addition or rename filled,
/// where that Entry stands there still (spec: EP-7).
///
/// "Another Entry" is any but the one the addition's Entry takes the place of:
/// a rename of that one is what [`take_over`](super::take_over::take_over)
/// follows. And it is another Entry whether the batch keeps it or removes it,
/// because both writes are to one path. An Entry that stood at the path before
/// the batch was prepared is not a rebase's to report: the batch was prepared
/// against it, and the candidate check says what is wrong with that — as it
/// says it of an addition meeting another writer's addition, the collision that
/// has always been (spec: EP-6).
pub(super) async fn check_contested(
    index: &dyn Index,
    batch: &PreparedBatch,
    renamed: &BTreeMap<EntryRef, &EntryPath>,
    intervening: &[JournalRecord],
) -> CommitResult<()> {
    let renamed_onto = !renamed.is_empty() && !batch.additions.is_empty();
    let filled = !batch.renames.is_empty()
        && intervening
            .iter()
            .any(|record| !record.renames().is_empty() || !record.additions().is_empty());
    if !renamed_onto && !filled {
        return Ok(());
    }
    let current = index.entries_under(None).await?;
    let standing: BTreeMap<&EntryPath, &EntryLocation> = current
        .iter()
        .map(|location| (location.path(), location))
        .collect();
    let renamed_away: BTreeSet<EntryRef> =
        batch.renames.iter().map(|rename| rename.entry).collect();

    for prepared in &batch.additions {
        let container_id = prepared.addition.container().id;
        for (position, entry) in prepared.addition.entries().iter().enumerate() {
            let number = number_at(position);
            let superseded = prepared.supersedes.get(&number).copied();
            // The path it is committed at: where the Entry it takes the place
            // of stands now, or the path it was prepared under.
            let path = superseded
                .and_then(|superseded| renamed.get(&superseded).copied())
                .unwrap_or(&entry.path);
            let Some(holder) = standing.get(path) else {
                continue;
            };
            if Some(EntryRef::of(holder)) == superseded {
                continue;
            }
            if let Some((record, Placed::ByRename)) = placed(intervening, holder) {
                return Err(CommitError::RebaseConflict {
                    pending: PendingChange::Addition(EntryRef::new(container_id, number)),
                    record,
                    rule: RebaseRule::RenamedOntoAddedPath,
                    path: path.clone(),
                });
            }
        }
    }
    for rename in &batch.renames {
        let Some(holder) = standing.get(&rename.path) else {
            continue;
        };
        if renamed_away.contains(&EntryRef::of(holder)) {
            continue;
        }
        if let Some((record, _)) = placed(intervening, holder) {
            return Err(CommitError::RebaseConflict {
                pending: PendingChange::Rename(rename.entry),
                record,
                rule: RebaseRule::DestinationTaken,
                path: rename.path.clone(),
            });
        }
    }
    Ok(())
}

/// How an Entry came to stand where it stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placed {
    /// A record added its Container.
    ByAddition,
    /// A record renamed it there.
    ByRename,
}

/// The latest intervening record that put `holder` at the path it stands at,
/// and how; `None` where it stood there before the first of them.
fn placed(intervening: &[JournalRecord], holder: &EntryLocation) -> Option<(Generation, Placed)> {
    let entry = EntryRef::of(holder);
    intervening.iter().rev().find_map(|record| {
        if record
            .renames()
            .iter()
            .any(|rename| EntryRef::renamed_by(rename) == entry && rename.path() == holder.path())
        {
            return Some((record.generation(), Placed::ByRename));
        }
        record
            .additions()
            .iter()
            .any(|addition| addition.container().id == holder.container_id)
            .then_some((record.generation(), Placed::ByAddition))
    })
}
