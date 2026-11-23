use coffret_model::EntryRename;

use crate::catalog_renames::{check_rename_set, RenameLookups};
use crate::entry_ref::EntryRef;
use crate::index::Index;
use crate::index_error::{IndexError, IndexResult};
use crate::rename::rename_refusal::RenameRefusal;
use crate::rename::renamed_path::RenamedPath;

/// What FM-15's rename-set check holds a set of renames to, read off the
/// catalog once (spec: FM-15, RV-5).
///
/// The check a commit asks again of its batch and every catalog asks of the
/// record, asked here first. The plan's own questions answer most of what it
/// asks, so what reaches a refusal here is a rule they do not cover — a derived
/// Entry stored outside the reserved namespace, above all — and it is the
/// rename's refusal rather than an error inside the commit.
///
/// What the check asks of the catalog is looked up for the renames it is read
/// for — the Entries of the Containers they name and the Entries standing at
/// their destinations — and a refusal is asked only of those renames, or of
/// some of them.
pub(super) struct Rules {
    lookups: RenameLookups,
}

impl Rules {
    /// The rules against the catalog as it stands, for `renamed` or any part
    /// of it.
    pub(super) async fn read(
        index: &dyn Index,
        renamed: &[(EntryRef, RenamedPath)],
    ) -> IndexResult<Self> {
        Ok(Self {
            lookups: RenameLookups::read(index, &renames_of(renamed)).await?,
        })
    }

    /// The refusal the check makes of `renamed`, if it makes one.
    pub(super) fn refusal(
        &self,
        renamed: &[(EntryRef, RenamedPath)],
    ) -> IndexResult<Option<RenameRefusal>> {
        let checked = check_rename_set(
            &renames_of(renamed),
            &[],
            &[],
            |container_id, entry_number| Ok(self.lookups.named(container_id, entry_number)),
            |path| {
                Ok(self
                    .lookups
                    .standing_at(path)
                    .map(|entry| (entry.container_id, entry.entry_number)))
            },
        );
        match checked {
            Ok(_) => Ok(None),
            Err(IndexError::RenameRefused { rule, .. }) => Ok(Some(RenameRefusal::Rule(rule))),
            Err(other) => Err(other),
        }
    }
}

/// `renamed` as the elements of a record's `renames`.
fn renames_of(renamed: &[(EntryRef, RenamedPath)]) -> Vec<EntryRename> {
    renamed
        .iter()
        .map(|(entry, one)| {
            EntryRename::new(entry.container_id, entry.entry_number, one.to.clone())
                .expect("refuse_outright kept the destination out of the derived namespace")
        })
        .collect()
}
