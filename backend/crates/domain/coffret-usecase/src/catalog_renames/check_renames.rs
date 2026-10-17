use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, EntryNumber, EntryPath, JournalRecord};

use super::{Named, RenamedEntry};
use crate::index_error::{IndexError, IndexResult, RenameRule};

/// Checks every rename `record` makes against a catalog its removals have
/// already left, and answers with what each one changes.
///
/// `named` answers which Entry a `(Container ID, entry number)` names in that
/// catalog, and `standing_at` which Entry stands at an Entry Path, by the same
/// pair. Neither may change anything: they are asked in whatever order the
/// checks need and a refusal leaves the catalog to the caller as it was.
///
/// The renames take effect all at once, so an Entry may take the path another
/// Entry of the same record leaves — two Entries swapping their names is one
/// valid record — and no two may take the same one (spec: EP-5). The record's
/// own additions come after and are never targets of its renames.
///
/// # Errors
///
/// [`IndexError::RenameRefused`] naming the first element that breaks a rule
/// and the rule it breaks, and whatever either lookup fails with.
pub fn check_renames(
    record: &JournalRecord,
    mut named: impl FnMut(ContainerId, EntryNumber) -> IndexResult<Named>,
    mut standing_at: impl FnMut(&EntryPath) -> IndexResult<Option<(ContainerId, EntryNumber)>>,
) -> IndexResult<Vec<RenamedEntry>> {
    let renames = record.renames();
    if renames.is_empty() {
        return Ok(Vec::new());
    }
    let refused = |element: usize, rule: RenameRule| IndexError::RenameRefused { element, rule };

    let mut renamed = Vec::with_capacity(renames.len());
    for (element, rename) in renames.iter().enumerate() {
        let container_id = rename.container_id();
        // The record's removals are sorted by Container ID (spec: FM-15).
        if record.removals().binary_search(&container_id).is_ok() {
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
    for addition in record.additions() {
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
