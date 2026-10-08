use std::collections::BTreeSet;

use coffret_model::{ContainerId, EntryPath};

use crate::delete::delete_error::DeleteResult;
use crate::delete::delete_selection::DeleteSelection;
use crate::delete::plan::{self, Plan};
use crate::delete::refused_pack::RefusedPack;
use crate::index::Index;

/// What deleting a selection would do to the Library, counted before it is
/// asked for.
///
/// Every number is the one [`delete_entries`](super::delete_entries) then
/// reports for the same selection over the same catalog, because both are
/// worked out by the same plan (spec: PK-9). They can differ only where the
/// Library changes in between, or where a Container the run reads does not
/// verify — which a preview, reading nothing, cannot know (spec: PK-10).
#[derive(Debug, Default)]
pub struct DeletePreview {
    /// How many Entries would leave the Library.
    pub entries: usize,
    /// Their total length, in plaintext bytes (spec: FM-9).
    pub bytes: u64,
    /// How many Containers would be removed outright: every Entry of each is
    /// named (spec: PK-9).
    pub removed: usize,
    /// How many Packs would be rebuilt by read-modify-replace around the
    /// Entries they keep (spec: PK-9, PK-10).
    pub rebuilt: usize,
    /// How many bytes those rebuilds read from Storage: every old Pack whole
    /// (spec: PK-10, PK-16).
    pub rebuild_read: u64,
    /// How many bytes the replacements they write weigh on Storage.
    pub rebuild_written: u64,
    /// The Packs the deletion would be refused for, because they keep Entries
    /// and the committed Keyring maps them to a key-lost marker (spec: PK-10,
    /// KL-17).
    pub refused: Vec<RefusedPack>,
    /// Named Entry Paths that hold no current Entry.
    pub missing: Vec<EntryPath>,
}

impl DeletePreview {
    /// The counts a plan comes to.
    fn of(plan: Plan) -> Result<Self, coffret_format::Error> {
        let deleted = plan
            .removals
            .iter()
            .flat_map(|whole| whole.entries.iter())
            .chain(plan.rebuilds.iter().flat_map(|partial| partial.deleted()));
        let (entries, bytes) = deleted.fold((0, 0), |(count, bytes), row| {
            (count + 1, bytes + row.extent.size())
        });
        let mut rebuild_written = 0;
        for partial in &plan.rebuilds {
            rebuild_written += partial.written_len()?;
        }
        Ok(Self {
            entries,
            bytes,
            removed: plan.removals.len(),
            rebuilt: plan.rebuilds.len(),
            rebuild_read: plan
                .rebuilds
                .iter()
                .map(|partial| partial.summary.ciphertext_len.get())
                .sum(),
            rebuild_written,
            refused: plan.refused,
            missing: plan.missing,
        })
    }
}

/// Counts what [`delete_entries`](super::delete_entries) would do with
/// `selection`, reading the catalog and nothing else.
///
/// No Storage, no key, no write: a person is told what a deletion would cost
/// before asking for one. The one fact the catalog does not hold is which
/// Containers the committed Keyring maps to a key-lost marker, and reading that
/// takes Storage and the Master Key (spec: KL-7). So it is handed in —
/// `key_lost` is what a caller already learned from an earlier run's outcome,
/// empty if it knows of none — and a Pack it does not name is counted as
/// rebuilt. A run over a Pack whose key turns out to be lost refuses it, and
/// says so (spec: PK-10, KL-17).
pub async fn preview_delete(
    index: &dyn Index,
    selection: &DeleteSelection,
    key_lost: &BTreeSet<ContainerId>,
) -> DeleteResult<DeletePreview> {
    let plan = plan::plan(index, selection, key_lost).await?;
    Ok(DeletePreview::of(plan)?)
}
