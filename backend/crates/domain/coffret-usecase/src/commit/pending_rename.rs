use coffret_model::EntryPath;

use crate::entry_ref::EntryRef;

/// One rename a batch makes: the Entry it gives a new current Entry Path, the
/// name it was prepared against, and the path it gives it (spec: FM-15).
///
/// The Entry is named by `(Container ID, entry number)`, which is what the
/// record will carry. The name it was prepared against is not carried by any
/// record — the catalog holds it (spec: FM-15) — and is kept here for one
/// reader: a rebase onto a head where another writer replaced the Entry's
/// Container. Whether the rename still has an Entry to rename there is
/// whether that name now names the replacement's Entry (spec: EP-7).
///
/// A folder rename is one of these per Entry it was prepared with, and no
/// more: a rebase never extends it to Entries another writer added under the
/// folder meanwhile (spec: EP-7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRename {
    /// The Entry it renames.
    pub entry: EntryRef,
    /// The Entry's current path in the catalog the batch was prepared
    /// against.
    pub from: EntryPath,
    /// The Entry Path the Entry stands at once the batch commits.
    pub path: EntryPath,
}
