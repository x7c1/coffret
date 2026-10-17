use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, EntryLocation, EntryPath, EntryRename};

use crate::catalog_renames::{check_rename_set, Named};
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
pub(super) struct Rules<'a> {
    containers: BTreeSet<ContainerId>,
    by_number: BTreeMap<EntryRef, &'a EntryLocation>,
    standing: BTreeMap<&'a EntryPath, EntryRef>,
}

impl<'a> Rules<'a> {
    /// The rules against the catalog `current` was read from, `current` being
    /// every current Entry in it.
    pub(super) async fn read(index: &dyn Index, current: &'a [EntryLocation]) -> IndexResult<Self> {
        Ok(Self {
            containers: index
                .containers_under(None)
                .await?
                .into_iter()
                .map(|container| container.id)
                .collect(),
            by_number: current
                .iter()
                .map(|location| (EntryRef::of(location), location))
                .collect(),
            standing: current
                .iter()
                .map(|location| (location.path(), EntryRef::of(location)))
                .collect(),
        })
    }

    /// The refusal the check makes of `renamed`, if it makes one.
    pub(super) fn refusal(
        &self,
        renamed: &[(EntryRef, RenamedPath)],
    ) -> IndexResult<Option<RenameRefusal>> {
        let renames = renamed
            .iter()
            .map(|(entry, one)| {
                EntryRename::new(entry.container_id, entry.entry_number, one.to.clone())
                    .expect("refuse_outright kept the destination out of the derived namespace")
            })
            .collect::<Vec<_>>();
        let checked = check_rename_set(
            &renames,
            &[],
            &[],
            |container_id, entry_number| {
                Ok(
                    match self
                        .by_number
                        .get(&EntryRef::new(container_id, entry_number))
                    {
                        Some(location) => Named::Entry(Box::new((*location).clone())),
                        None if self.containers.contains(&container_id) => Named::NoEntry,
                        None => Named::NoContainer,
                    },
                )
            },
            |path| {
                Ok(self
                    .standing
                    .get(path)
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
