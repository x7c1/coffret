use coffret_model::{ContainerId, EntryNumber, EntryPath, JournalRecord};

use super::{check_rename_set, Named, RenamedEntry};
use crate::index_error::IndexResult;

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
/// [`IndexError::RenameRefused`](crate::IndexError::RenameRefused)
/// naming the first element that breaks a rule and the rule it breaks, and
/// whatever either lookup fails with.
pub fn check_renames(
    record: &JournalRecord,
    named: impl FnMut(ContainerId, EntryNumber) -> IndexResult<Named>,
    standing_at: impl FnMut(&EntryPath) -> IndexResult<Option<(ContainerId, EntryNumber)>>,
) -> IndexResult<Vec<RenamedEntry>> {
    check_rename_set(
        record.renames(),
        record.removals(),
        record.additions(),
        named,
        standing_at,
    )
}
