use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerAddition, ContainerId, EntryNumber, EntryPath, EntryRename};

use super::{Named, RenamedEntry};
use crate::index_error::{IndexError, IndexResult, RenameRule};

/// [`check_renames`](fn@super::check_renames) over the three collections a
/// record carries, for a writer that holds them before any record exists.
///
/// A commit asks the same questions of the batch it is about to write as every
/// catalog asks of the record once it is written, so that a record it creates
/// is one each catalog applies (spec: FM-15, CP-1). `removals` is in Container
/// ID order, as a record carries it (spec: FM-15); `renames` and `additions`
/// may be in any.
///
/// # Errors
///
/// [`check_renames`](fn@super::check_renames)'s, with `element` the position in
/// `renames` as given.
pub fn check_rename_set(
    renames: &[EntryRename],
    removals: &[ContainerId],
    additions: &[ContainerAddition],
    mut named: impl FnMut(ContainerId, EntryNumber) -> IndexResult<Named>,
    mut standing_at: impl FnMut(&EntryPath) -> IndexResult<Option<(ContainerId, EntryNumber)>>,
) -> IndexResult<Vec<RenamedEntry>> {
    if renames.is_empty() {
        return Ok(Vec::new());
    }
    let refused = |element: usize, rule: RenameRule| IndexError::RenameRefused { element, rule };

    let mut renamed = Vec::with_capacity(renames.len());
    for (element, rename) in renames.iter().enumerate() {
        let container_id = rename.container_id();
        // The record's removals are sorted by Container ID (spec: FM-15).
        if removals.binary_search(&container_id).is_ok() {
            return Err(refused(
                element,
                RenameRule::ContainerRemoved { container_id },
            ));
        }
        // The removals are out of the catalog already, and the Container is
        // none of them: current now is current immediately before the record.
        let before = match named(container_id, rename.entry_number())? {
            Named::NoContainer => {
                return Err(refused(
                    element,
                    RenameRule::ContainerNotCurrent { container_id },
                ))
            }
            Named::NoEntry => {
                return Err(refused(
                    element,
                    RenameRule::EntryNumberOutsideTable {
                        container_id,
                        entry_number: rename.entry_number().get(),
                    },
                ))
            }
            Named::Entry(before) => *before,
        };
        if before.path() == rename.path() {
            return Err(refused(element, RenameRule::PathUnchanged));
        }
        if before.entry.derived_from.is_some() || before.path().is_derived() {
            return Err(refused(element, RenameRule::DerivedEntry));
        }
        renamed.push(RenamedEntry {
            before,
            path: rename.path().clone(),
        });
    }

    // Uniqueness over the state the renames leave: an Entry renamed by this
    // record vacates its path, so only an Entry it leaves where it is can be in
    // a rename's way.
    let moving: BTreeSet<(ContainerId, EntryNumber)> =
        renames.iter().map(|rename| rename.order_key()).collect();
    let mut claimed: BTreeMap<&EntryPath, usize> = BTreeMap::new();
    for (element, rename) in renames.iter().enumerate() {
        if let Some(&earlier) = claimed.get(rename.path()) {
            return Err(refused(element, RenameRule::PathClaimedTwice { earlier }));
        }
        claimed.insert(rename.path(), element);
        if let Some(standing) = standing_at(rename.path())? {
            if !moving.contains(&standing) {
                return Err(refused(element, RenameRule::PathTaken));
            }
        }
    }

    // And over the additions that come after.
    for addition in additions {
        for entry in addition.entries() {
            if let Some(&element) = claimed.get(&entry.path) {
                return Err(refused(
                    element,
                    RenameRule::PathAdded {
                        container_id: addition.container().id,
                    },
                ));
            }
        }
    }
    Ok(renamed)
}
