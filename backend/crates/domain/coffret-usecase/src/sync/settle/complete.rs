use std::collections::{BTreeMap, BTreeSet};

use coffret_model::{ContainerId, EntryLocation};
use tracing::info;

use crate::device_state::{DeviceTime, LocalObservation, PendingRow};
use crate::entry_ref::EntryRef;
use crate::index::Index;
use crate::spool::Spool;
use crate::sync::settled::Settled;
use crate::sync::sync_error::SyncResult;

/// Whether one row is the commit-landed-refresh-did-not case rather than
/// something to reclaim.
///
/// Both halves of the test have to hold, and the state half is not implied by
/// the membership half. A row still
/// [`Spooling`](crate::device_state::SpoolState::Spooling) is a spool this
/// device announced and never finished, so nothing ever uploaded it and no record
/// can name it: a current Container of that ID would be some other run's, and
/// completing this row against it would mark Entries present that this device
/// never put on disk (spec: EP-10). Disposing of it instead is OC-2's posture
/// over this device's own provenance.
///
/// One function rather than a test written twice, because
/// [`materialized`] has to pick out exactly the rows the loop will complete —
/// two spellings of that could drift into a walk that gathers Entries nothing
/// consumes, or a completion with no Entries to record.
pub(super) fn completes(row: &PendingRow, current: &BTreeSet<ContainerId>) -> bool {
    row.state.is_spooled() && current.contains(&row.container_id)
}

/// The current Entries of every pending Container that turned out to be current.
///
/// These are the files the interrupted run put on disk while producing the
/// batch: the record carries each new Container's entry table (spec: CP-11), and
/// its captured path, `size`, and `mtime` are exactly what that run handed the
/// refresh it never completed. The captured path and not the current one,
/// because that is where the file stands: an Entry a rename has given another
/// path since — a rebase taking over one another writer renamed, or a rename
/// committed after it — stands elsewhere, and its file waits to be relocated
/// there (spec: EP-7, EP-10). Reading them out of the Index rather than off Storage is
/// what keeps this from opening a Container.
///
/// Taking the Container's Entries *as* the materialized files is sound because
/// of where these rows come from and nowhere else: a pending row is written by a
/// spool step alone, and one that says it
/// [`materializes`](PendingRow::materializes) is for a Container this device
/// built out of local files it holds — a one-file Container a sync drew from
/// one of them, or a Pack a freeze drew from several (spec: PK-7, PK-15) — so
/// every Entry it holds is a file this device put on disk. A Pack a deletion
/// rebuilt by read-modify-replace carries Entries forward off Storage instead
/// (spec: PK-10), and its row says so, so it contributes none.
///
/// What a commit adds is otherwise no evidence of that — a repack commits
/// Containers whose Entries the device may never have held, which is why
/// [`CommittedBatch`](crate::CommittedBatch) names the materialized files
/// rather than leaving them to be read off the additions.
///
/// One walk of the current Entries answers every completion, and it is walked at
/// all only where there is one to answer: the whole listing is not a hot path
/// when the ordinary run leaves this function unreached.
pub(super) async fn materialized(
    index: &dyn Index,
    pending: &[PendingRow],
    current: &BTreeSet<ContainerId>,
) -> SyncResult<BTreeMap<ContainerId, Vec<EntryLocation>>> {
    let completing: BTreeSet<ContainerId> = pending
        .iter()
        .filter(|row| completes(row, current) && row.materializes)
        .map(|row| row.container_id)
        .collect();
    let mut entries: BTreeMap<ContainerId, Vec<EntryLocation>> = BTreeMap::new();
    if completing.is_empty() {
        return Ok(entries);
    }
    for location in index.entries_under(None).await? {
        if completing.contains(&location.container_id) {
            entries
                .entry(location.container_id)
                .or_default()
                .push(location);
        }
    }
    Ok(entries)
}

/// Completes the bookkeeping of a commit that landed and whose refresh did not
/// (spec: OC-7).
///
/// The object is the Library's and is left exactly where it is. Everything else
/// the interrupted refresh would have done is done here: the `entries` it is
/// handed, if any, are marked present, stamped with this run's clock because the
/// moment the earlier run looked is not recorded anywhere (spec: EP-10), and the
/// spool and the row go because a committed Container is no longer a candidate
/// for cleanup (spec: OC-2).
pub(super) async fn complete(
    index: &dyn Index,
    spool: &dyn Spool,
    now: DeviceTime,
    row: PendingRow,
    entries: Option<Vec<EntryLocation>>,
) -> SyncResult<Settled> {
    let entries = entries.unwrap_or_default();
    for location in &entries {
        let entry = &location.entry;
        // Recorded where the file stands, as a copy of the Entry wherever that
        // stands now: the Index has it wait to follow an Entry a rename gave
        // another path (spec: EP-10).
        index
            .mark_present(LocalObservation {
                path: entry.captured_path.clone(),
                size: entry.extent.size(),
                mtime: entry.mtime,
                at: now,
                entry: EntryRef::of(location),
                hash: Some(entry.hash),
            })
            .await?;
    }

    spool.discard(&row.spool_path).await?;
    index.clear_pending_row(row.container_id).await?;
    info!(
        container = %row.container_id,
        batch = %row.batch,
        entries = entries.len(),
        "completed the bookkeeping of a Container whose commit landed and whose refresh did not",
    );
    Ok(Settled::Completed {
        container_id: row.container_id,
        entries: entries.len(),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use coffret_model::{
        ContainerAddition, ContainerKind, ContainerSummary, ContentHash, EntryMetadata,
        JournalRecord, KeyringCommitment, MasterKeyEpoch, Mtime,
    };

    use super::*;
    use crate::ciphertext_len_claims::ciphertext_len;
    use crate::committed_batch::CommittedBatch;
    use crate::device_state::{BatchId, SpoolState};
    use crate::entry_extents::entry_extent;
    use crate::entry_paths::entry_path;
    use crate::generations::generation;
    use crate::in_memory_fs::InMemoryFs;
    use crate::in_memory_index::InMemoryIndex;

    /// A one-file Container `seed` whose Entry stands at `path`, captured
    /// under `captured`.
    fn one_file(seed: u8, path: &str, captured: &str) -> ContainerAddition {
        ContainerAddition::new(
            ContainerSummary {
                id: ContainerId::from_bytes([seed; ContainerId::BYTE_LEN]),
                kind: ContainerKind::OneFile,
                ciphertext_hash: ContentHash::from_bytes([seed; ContentHash::BYTE_LEN]),
                ciphertext_len: ciphertext_len(100 + 64),
                object_ref: None,
            },
            vec![EntryMetadata {
                path: entry_path(path),
                captured_path: entry_path(captured),
                extent: entry_extent(0, 100),
                mtime: Mtime::from_unix_seconds(1_700_000_000),
                btime: None,
                hash: ContentHash::from_bytes([seed.wrapping_add(1); ContentHash::BYTE_LEN]),
                derived_from: None,
                mime: None,
            }],
        )
        .expect("one Entry tiles its Container's stream")
    }

    /// The row an interrupted sync left for Container `seed`: spooled,
    /// uploaded, its commit attempted, built out of this device's files.
    fn pending(seed: u8) -> PendingRow {
        PendingRow {
            commit_attempted: true,
            container_id: ContainerId::from_bytes([seed; ContainerId::BYTE_LEN]),
            spool_path: PathBuf::from(format!("/spool/{seed}.spool")),
            batch: BatchId::new("an-interrupted-sync"),
            created_at: DeviceTime::from_unix_seconds(1_700_000_000),
            state: SpoolState::Spooled(None),
            materializes: true,
        }
    }

    // EP-7, EP-10: a Container whose commit landed, rebased over another
    // writer's rename of the Entry it replaces, holds that Entry at the renamed
    // path while this device's file stands at the path it was captured from.
    // Completing the interrupted bookkeeping records that file where it
    // stands, waiting to follow the Entry, and claims no file at the renamed
    // path.
    #[tokio::test]
    async fn an_entry_taken_over_at_a_renamed_path_waits_where_its_file_stands() {
        let index = InMemoryIndex::new();
        let record = JournalRecord::canonical(
            generation(1),
            // A first commit succeeds the head the Library was created with.
            Some(generation(0)),
            MasterKeyEpoch::FIRST,
            KeyringCommitment::new(generation(1), 3, "beef")
                .expect("a lowercase hex digest and a non-zero count are a commitment"),
            None,
            None,
            vec![
                one_file(1, "albums/b.jpg", "albums/a.jpg"),
                one_file(2, "albums/c.jpg", "albums/c.jpg"),
            ],
            Vec::new(),
        )
        .expect("the record is one a commit could write");
        index
            .refresh(CommittedBatch {
                record,
                materialized: Vec::new(),
                released: Vec::new(),
            })
            .await
            .expect("applying the record must succeed");
        let rows = [pending(1), pending(2)];
        let current: BTreeSet<ContainerId> = rows.iter().map(|row| row.container_id).collect();

        let mut entries = materialized(&index, &rows, &current)
            .await
            .expect("reading the catalog must succeed");
        let spool = InMemoryFs::new();
        for row in rows {
            let held = entries.remove(&row.container_id);
            complete(&index, &spool, DeviceTime::from_unix_seconds(1), row, held)
                .await
                .expect("completing the interrupted bookkeeping must succeed");
        }

        let row = |text| {
            let index = &index;
            async move {
                index
                    .local_entry_at(&entry_path(text))
                    .await
                    .expect("reading a local file's row must succeed")
            }
        };
        let taken_over = row("albums/a.jpg")
            .await
            .expect("the file the update was drawn from is recorded where it stands");
        assert_eq!(taken_over.relocate_to, Some(entry_path("albums/b.jpg")));
        assert!(
            row("albums/b.jpg").await.is_none(),
            "no file is claimed at the renamed path",
        );
        assert_eq!(
            row("albums/c.jpg")
                .await
                .expect("the Entry standing where its file was captured is recorded")
                .relocate_to,
            None,
        );
    }
}
