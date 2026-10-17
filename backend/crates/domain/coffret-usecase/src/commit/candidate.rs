use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerAddition, ContainerId, EntryLocation, EntryPath, EntryRename};

use crate::catalog_renames::{check_rename_set, Named, RenamedEntry};
use crate::commit::commit_error::{CommitError, CommitResult};
use crate::commit::entry_ref::EntryRef;
use crate::commit::prepared_batch::PreparedBatch;
use crate::index::Index;
use crate::index_error::IndexError;

/// Refuses a batch that cannot commit onto the head the Index stands at: one
/// that removes a Container that is no longer current (spec: CP-18), one with
/// a rename the head does not admit (spec: FM-15), or one whose commit would
/// put two current Entries at one Entry Path (spec: EP-5, EP-6).
///
/// The removals are checked first, because the path check takes them as given:
/// it lets an Entry leave with a Container the batch removes, and a Container
/// another writer already removed has no Entries left to leave. A batch
/// prepared against a head where that Container was current would otherwise
/// pass, and its commit would silently undo the other writer's removal — a
/// replacement landing for a file somebody deleted, a rebuilt Pack bringing
/// back Entries somebody deleted with it (spec: CP-7).
///
/// The path order is the one EP-6 and FM-15 fix and it is not incidental:
/// every Entry owned by the batch's removals leaves the current path map
/// first, the Entries it renames take their new paths all at once, and the
/// additions enter after. That is what lets a path pass from a replaced
/// Container to its replacement inside one batch, and it is why a collision
/// with a Container the batch is removing is no collision at all.
///
/// This runs before anything is written, because a batch that cannot commit
/// should not have left a Keyring candidate behind for orphan cleanup to reason
/// about. A writer that later loses the commit race runs it again against the
/// new head, which is how two concurrent writes become an explicit conflict
/// instead of last-write-wins (spec: CP-7, CP-18, EP-7).
pub(super) async fn check(index: &dyn Index, batch: &PreparedBatch) -> CommitResult<()> {
    let removed: BTreeSet<ContainerId> = batch.removals.iter().copied().collect();
    check_removals(index, &removed).await?;

    let current = index.entries_under(None).await?;
    let renamed = check_renames(index, batch, &removed, &current).await?;

    let mut paths: BTreeSet<&EntryPath> = current
        .iter()
        .filter(|location| !removed.contains(&location.container_id))
        .map(|location| &location.entry.path)
        .collect();
    // The renames apply together, after the removals and before the additions
    // (spec: FM-15); that they leave one Entry per path is what
    // [`check_renames`] has just said.
    for one in &renamed {
        paths.remove(one.from());
    }
    for one in &renamed {
        paths.insert(&one.path);
    }

    for prepared in &batch.additions {
        for entry in prepared.addition.entries() {
            if !paths.insert(&entry.path) {
                return Err(CommitError::EntryPathCollision {
                    path: entry.path.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Holds the batch's renames to the rules every catalog holds a record's
/// renames to, against the head the batch commits onto, and answers with what
/// each one changes (spec: FM-15).
///
/// The record the batch commits is applied by this device's own catalog right
/// after it is created and by every other device's on catch-up, and one whose
/// renames a catalog does not admit is refused whole there (spec: RV-5): it
/// would be a commit no device could follow. So the same rules are asked here,
/// before anything is written, over the same three collections.
async fn check_renames(
    index: &dyn Index,
    batch: &PreparedBatch,
    removed: &BTreeSet<ContainerId>,
    current: &[EntryLocation],
) -> CommitResult<Vec<RenamedEntry>> {
    if batch.renames.is_empty() {
        return Ok(Vec::new());
    }
    let renames = batch
        .renames
        .iter()
        .map(|rename| {
            EntryRename::new(
                rename.entry.container_id,
                rename.entry.entry_number,
                rename.path.clone(),
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|cause| CommitError::UnwritableControlValue { cause })?;
    let containers: BTreeSet<ContainerId> = index
        .containers_under(None)
        .await?
        .into_iter()
        .map(|container| container.id)
        .collect();
    let by_number: BTreeMap<EntryRef, &EntryLocation> = current
        .iter()
        .map(|location| (EntryRef::of(location), location))
        .collect();
    let standing: BTreeMap<&EntryPath, EntryRef> = current
        .iter()
        .filter(|location| !removed.contains(&location.container_id))
        .map(|location| (location.path(), EntryRef::of(location)))
        .collect();
    let removals: Vec<ContainerId> = removed.iter().copied().collect();
    let additions: Vec<ContainerAddition> = batch
        .additions
        .iter()
        .map(|prepared| prepared.addition.clone())
        .collect();

    check_rename_set(
        &renames,
        &removals,
        &additions,
        |container_id, entry_number| {
            Ok(
                match by_number.get(&EntryRef::new(container_id, entry_number)) {
                    Some(location) => Named::Entry(Box::new((*location).clone())),
                    None if containers.contains(&container_id) => Named::NoEntry,
                    None => Named::NoContainer,
                },
            )
        },
        |path| {
            Ok(standing
                .get(path)
                .map(|entry| (entry.container_id, entry.entry_number)))
        },
    )
    .map_err(|refusal| match refusal {
        IndexError::RenameRefused { element, rule } => CommitError::RenameRefused { element, rule },
        other => other.into(),
    })
}

/// Refuses removals that name a Container the current set no longer holds,
/// naming every one of them (spec: CP-18).
///
/// A batch that removes nothing costs no walk of the Index.
async fn check_removals(index: &dyn Index, removed: &BTreeSet<ContainerId>) -> CommitResult<()> {
    if removed.is_empty() {
        return Ok(());
    }
    let current: BTreeSet<ContainerId> = index
        .containers_under(None)
        .await?
        .into_iter()
        .map(|container| container.id)
        .collect();
    let gone: Vec<ContainerId> = removed.difference(&current).copied().collect();
    if gone.is_empty() {
        Ok(())
    } else {
        Err(CommitError::RemovalNotCurrent {
            container_ids: gone,
        })
    }
}
