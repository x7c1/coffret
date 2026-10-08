use std::collections::{BTreeMap, BTreeSet};

use coffret_format::{ChunkSize, ContainerFootprint, EntryPlan};
use coffret_model::{ContainerId, ContainerSummary, EntryMetadata, EntryPath};

use crate::delete::delete_selection::DeleteSelection;
use crate::delete::pack_refusal::PackRefusal;
use crate::delete::refused_pack::RefusedPack;
use crate::index::Index;
use crate::index_error::{IndexError, IndexResult};

/// What a deletion of one selection does to the Library, worked out from the
/// catalog alone (spec: PK-9).
///
/// It is the same answer for the run and for the preview, which is the point of
/// having one: the preview can only say what the run will do if the two ask the
/// same question of the same catalog. Nothing here reads Storage or needs a
/// key; which Containers carry a key-lost marker is the one thing the catalog
/// does not know, and is handed in.
#[derive(Debug, Default)]
pub(super) struct Plan {
    /// Containers every Entry of which is named: removed outright, whatever
    /// their kind and whether or not their key is lost (spec: PK-9, KL-17).
    pub(super) removals: Vec<Whole>,
    /// Containers that also keep Entries the request did not name, to be
    /// replaced by read-modify-replace (spec: PK-9, PK-10).
    pub(super) rebuilds: Vec<Partial>,
    /// Containers that would need read-modify-replace and whose key is lost,
    /// so the deletion is refused for them (spec: PK-10, KL-17).
    pub(super) refused: Vec<RefusedPack>,
    /// Named Entry Paths that hold no current Entry.
    pub(super) missing: Vec<EntryPath>,
}

/// A Container the deletion removes outright.
#[derive(Debug)]
pub(super) struct Whole {
    pub(super) summary: ContainerSummary,
    /// Every Entry it holds, all of them named.
    pub(super) entries: Vec<EntryMetadata>,
}

/// A Container the deletion replaces by read-modify-replace.
#[derive(Debug)]
pub(super) struct Partial {
    pub(super) summary: ContainerSummary,
    /// Its rows as the catalog records them, in stream order (spec: FM-9).
    pub(super) table: Vec<EntryMetadata>,
    /// Which of those rows the replacement keeps: the ones not named.
    pub(super) keep: Vec<bool>,
}

impl Partial {
    /// The rows the request named, which leave the Library with this
    /// Container's replacement.
    pub(super) fn deleted(&self) -> impl Iterator<Item = &EntryMetadata> {
        self.rows(false)
    }

    /// The rows the replacement carries forward.
    pub(super) fn kept(&self) -> impl Iterator<Item = &EntryMetadata> {
        self.rows(true)
    }

    fn rows(&self, kept: bool) -> impl Iterator<Item = &EntryMetadata> {
        self.table
            .iter()
            .zip(&self.keep)
            .filter(move |(_, keep)| **keep == kept)
            .map(|(row, _)| row)
    }

    /// The refusal naming this Container's kept and spared Entries.
    pub(super) fn refusal(&self, reason: PackRefusal) -> RefusedPack {
        RefusedPack {
            container_id: self.summary.id,
            kept: self.kept().map(|row| row.path.clone()).collect(),
            spared: self.deleted().map(|row| row.path.clone()).collect(),
            reason,
        }
    }

    /// How many bytes the replacement's object will weigh on Storage.
    ///
    /// Worked out from the kept rows by the format layer's own measure, which
    /// is the measure the writer then writes to: the replacement has exactly
    /// these rows, in this order, in the old Container's kind (spec: PK-10,
    /// PK-15).
    pub(super) fn written_len(&self) -> Result<u64, coffret_format::Error> {
        let plans: Vec<EntryPlan> = self.kept().map(EntryPlan::from).collect();
        Ok(ContainerFootprint::of(self.summary.kind, &plans)?.stored_len(ChunkSize::DEFAULT))
    }
}

/// Works out what deleting `selection` does, given which Containers carry a
/// key-lost marker.
///
/// Each Container holding a named Entry is looked at whole, because what
/// happens to it depends on what else it holds: everything named, and it is
/// removed; something left, and it is rebuilt around what is left — or, where
/// its key is lost and nothing can be read back to rebuild from, refused
/// (spec: PK-9, PK-10, KL-17).
pub(super) async fn plan(
    index: &dyn Index,
    selection: &DeleteSelection,
    key_lost: &BTreeSet<ContainerId>,
) -> IndexResult<Plan> {
    let mut named: BTreeSet<EntryPath> = BTreeSet::new();
    let mut touched: BTreeSet<ContainerId> = BTreeSet::new();
    let mut missing = Vec::new();
    for path in &selection.paths {
        match index.entry_at(path).await? {
            Some(location) => {
                touched.insert(location.container_id);
                named.insert(location.entry.path);
            }
            None => missing.push(path.clone()),
        }
    }
    if let Some(folder) = &selection.folder {
        for location in index.entries_under(Some(folder)).await? {
            touched.insert(location.container_id);
            named.insert(location.entry.path);
        }
    }
    if touched.is_empty() {
        return Ok(Plan {
            missing,
            ..Plan::default()
        });
    }

    // A Pack's other Entries may lie anywhere in the namespace — path ranges of
    // Packs from different invocations overlap and interleave (spec: PK-8) — so
    // the tables are read off the whole catalog rather than off the selection.
    let mut tables: BTreeMap<ContainerId, Vec<EntryMetadata>> = BTreeMap::new();
    for location in index.entries_under(None).await? {
        if touched.contains(&location.container_id) {
            tables
                .entry(location.container_id)
                .or_default()
                .push(location.entry);
        }
    }
    let summaries: BTreeMap<ContainerId, ContainerSummary> = index
        .containers_under(None)
        .await?
        .into_iter()
        .filter(|summary| touched.contains(&summary.id))
        .map(|summary| (summary.id, summary))
        .collect();

    let mut plan = Plan {
        missing,
        ..Plan::default()
    };
    for container_id in touched {
        let summary = summaries
            .get(&container_id)
            .cloned()
            .ok_or_else(|| unheld(container_id))?;
        let mut table = tables.remove(&container_id).unwrap_or_default();
        // The catalog answers in path order; a Container's own order is the
        // order its rows occupy its stream (spec: FM-9).
        table.sort_by_key(|row| row.extent.range().start);
        let keep: Vec<bool> = table.iter().map(|row| !named.contains(&row.path)).collect();

        if !keep.iter().any(|keep| *keep) {
            plan.removals.push(Whole {
                summary,
                entries: table,
            });
            continue;
        }
        let partial = Partial {
            summary,
            table,
            keep,
        };
        if key_lost.contains(&container_id) {
            plan.refused.push(partial.refusal(PackRefusal::KeyLost));
        } else {
            plan.rebuilds.push(partial);
        }
    }
    Ok(plan)
}

/// A catalog that names an Entry in a Container it does not hold.
///
/// Every Entry names a Container the catalog holds — the stored form refuses
/// anything else — so this is a catalog that has stopped answering
/// consistently, and it is reported as one rather than read as a deletion of
/// nothing.
fn unheld(container_id: ContainerId) -> IndexError {
    IndexError::UnreadableCatalog {
        operation: "reading the Containers a deletion touches",
        cause: format!("an Entry names Container {container_id}, which the catalog does not hold")
            .into(),
    }
}
