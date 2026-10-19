use std::collections::{BTreeMap, BTreeSet};

use coffret_format::{ChunkSize, ContainerFootprint, EntryPlan};
use coffret_model::{
    ContainerId, ContainerSummary, DepositedAt, EntryLocation, EntryMetadata, EntryPath,
};

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
    /// When each row's content was deposited, row for row: what the
    /// replacement's kept Entries carry forward (spec: PK-10, FM-15).
    pub(super) deposit_times: Vec<DepositedAt>,
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
    let mut tables: BTreeMap<ContainerId, Vec<EntryLocation>> = BTreeMap::new();
    for location in index.entries_under(None).await? {
        if touched.contains(&location.container_id) {
            tables
                .entry(location.container_id)
                .or_default()
                .push(location);
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
        let mut numbered = tables.remove(&container_id).unwrap_or_default();
        // The catalog answers in path order; a Container's own order is its
        // entry numbers (spec: FM-9). Offsets cannot stand in for them: Entries
        // of length zero share one, and a rename has made path order no guide.
        numbered.sort_by_key(|location| location.entry_number);
        let (table, deposit_times): (Vec<EntryMetadata>, Vec<DepositedAt>) = numbered
            .into_iter()
            .map(|location| (location.entry, location.deposited_at))
            .unzip();
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
            deposit_times,
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
        cause: Box::<dyn std::error::Error + Send + Sync>::from(format!(
            "an Entry names Container {container_id}, which the catalog does not hold"
        ))
        .into(),
    }
}

#[cfg(test)]
mod tests {
    use coffret_model::{
        ContainerAddition, ContainerKind, ContentHash, EntryNumber, EntryRename, JournalRecord,
        KeyringCommitment, MasterKeyEpoch, Mtime,
    };

    use super::*;
    use crate::ciphertext_len_claims::ciphertext_len;
    use crate::entry_extents::entry_extent;
    use crate::entry_paths::entry_path;
    use crate::generations::generation;
    use crate::in_memory_index::InMemoryIndex;

    fn pack() -> ContainerId {
        ContainerId::from_bytes([0x11; ContainerId::BYTE_LEN])
    }

    fn number(n: u64) -> EntryNumber {
        EntryNumber::new(n).expect("a small number is an entry number")
    }

    /// The head at `number`, adding `additions` and renaming `renames`.
    fn record(
        number: u64,
        additions: Vec<ContainerAddition>,
        renames: Vec<EntryRename>,
    ) -> JournalRecord {
        JournalRecord::canonical(
            generation(number),
            number.checked_sub(1).map(generation),
            MasterKeyEpoch::FIRST,
            KeyringCommitment::new(generation(number), 3, "beef")
                .expect("a lowercase hex digest and a non-zero count are a commitment"),
            None,
            None,
            additions,
            vec![],
        )
        .and_then(|record| record.with_renames(renames))
        .expect("a fixture holds a record a commit could have written")
    }

    /// When the row at `position` of [`addition`]'s table was deposited: a
    /// second of its own for each row.
    fn deposited_at(position: usize) -> DepositedAt {
        DepositedAt::from_unix_seconds(1_760_000_000 + position as i64)
    }

    /// A Pack whose table is `rows` — `(path, offset, size)` — in entry
    /// number order.
    fn addition(rows: &[(&str, u64, u64)]) -> ContainerAddition {
        let entries = rows
            .iter()
            .map(|(text, offset, size)| EntryMetadata {
                path: entry_path(*text),
                captured_path: entry_path(*text),
                extent: entry_extent(*offset, *size),
                mtime: Mtime::from_unix_seconds(1_700_000_000),
                btime: None,
                hash: ContentHash::from_bytes([0x22; ContentHash::BYTE_LEN]),
                derived_from: None,
                mime: None,
            })
            .collect();
        ContainerAddition::new(
            ContainerSummary {
                id: pack(),
                kind: ContainerKind::Pack,
                ciphertext_hash: ContentHash::from_bytes([0x11; ContentHash::BYTE_LEN]),
                ciphertext_len: ciphertext_len(164),
                object_ref: None,
            },
            entries,
            (0..rows.len()).map(deposited_at).collect(),
        )
        .expect("the fixture's table tiles its stream")
    }

    // FM-9, PK-10: a rebuild's table is the old Pack's in entry number order,
    // which is what the replacement's record of the Entries it carries forward
    // counts by. Three Entries begin at offset 0 — two of them empty — and a
    // rename has sent the third to the front of path order, so neither offsets
    // nor paths would recover the order the Pack holds them in.
    #[tokio::test]
    async fn a_rebuild_table_follows_entry_numbers_past_shared_offsets_and_a_rename() {
        let index = InMemoryIndex::new();
        let table = [
            ("albums/c.jpg", 0, 0),
            ("albums/a.jpg", 0, 0),
            ("albums/b.jpg", 0, 100),
        ];
        index
            .apply(record(0, vec![addition(&table)], vec![]))
            .await
            .expect("applying the Pack's record must succeed");
        let renamed = EntryRename::new(pack(), number(2), entry_path("albums/0.jpg"))
            .expect("an ordinary path is a rename target");
        index
            .apply(record(1, vec![], vec![renamed]))
            .await
            .expect("applying the rename must succeed");

        let selection = DeleteSelection::paths([entry_path("albums/a.jpg")].into());
        let plan = plan(&index, &selection, &BTreeSet::new())
            .await
            .expect("planning from the catalog must succeed");

        assert!(plan.removals.is_empty() && plan.refused.is_empty());
        let [rebuild] = plan.rebuilds.as_slice() else {
            panic!("one Pack is rebuilt, got {:?}", plan.rebuilds);
        };
        let rows: Vec<(&str, &str, u64)> = rebuild
            .table
            .iter()
            .map(|row| {
                (
                    row.path.as_str(),
                    row.captured_path.as_str(),
                    row.extent.size(),
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("albums/c.jpg", "albums/c.jpg", 0),
                ("albums/a.jpg", "albums/a.jpg", 0),
                ("albums/0.jpg", "albums/b.jpg", 100),
            ],
            "the rows stand in entry number order",
        );
        assert_eq!(
            rebuild.deposit_times,
            [deposited_at(0), deposited_at(1), deposited_at(2)],
            "each row's deposit time stands beside it, in the same order",
        );
        assert_eq!(rebuild.keep, [true, false, true]);
        assert_eq!(
            rebuild
                .kept()
                .map(|row| row.path.as_str())
                .collect::<Vec<_>>(),
            ["albums/c.jpg", "albums/0.jpg"],
        );
    }
}
