use coffret_model::{ContainerId, ContentHash, EntryPath, Generation, Mtime, ObjectRef};
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::device_state::{
    AttemptedSlot, BatchId, DeviceTime, HeldParcel, LocalEntryState, LocalObservation, PendingRow,
    SpoolState,
};
use coffret_usecase::{Index, IndexError};

mod support;

use support::{snapshot, stamp_of};

#[tokio::test]
async fn separate_connections_cannot_settle_a_live_producer() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let first = SqliteIndex::open(&path).unwrap();
    let second = SqliteIndex::open(&path).unwrap();
    let owner = first.own_pending_rows().await.unwrap();
    assert!(matches!(
        second.own_pending_rows().await,
        Err(IndexError::PendingRowsBusy { .. })
    ));
    drop(owner);
    let _next = second.own_pending_rows().await.unwrap();
}

#[tokio::test]
async fn old_provenance_is_preserved_as_uncertain_after_reopening() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = SqliteIndex::open(&path).unwrap();
    let row = PendingRow {
        container_id: ContainerId::from_bytes([9; 16]),
        spool_path: dir.path().join("spool"),
        batch: BatchId::new("interrupted"),
        created_at: DeviceTime::from_unix_seconds(1),
        state: SpoolState::Spooled(Some(ObjectRef::new("uploaded"))),
        commit_attempted: false,
        materializes: false,
    };
    index.record_pending_row(row.clone()).await.unwrap();
    drop(index);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "ALTER TABLE pending_rows DROP COLUMN commit_attempted; \
             ALTER TABLE pending_rows DROP COLUMN materializes; \
             ALTER TABLE local_entries DROP COLUMN hash; DROP TABLE held_parcels; \
             DROP TABLE attempted_slots; ALTER TABLE entries DROP COLUMN captured_path; \
             ALTER TABLE local_entries DROP COLUMN relocate_to; PRAGMA user_version = 7;",
        )
        .unwrap();
    drop(connection);
    let reopened = SqliteIndex::open(&path).unwrap();
    // Every row a build before layout 9 wrote names a Container built out of
    // this device's own files, because nothing else was spooled then
    // (spec: OC-7).
    let expected = PendingRow {
        commit_attempted: true,
        materializes: true,
        ..row
    };
    assert_eq!(
        reopened.pending_rows().await.unwrap().as_slice(),
        std::slice::from_ref(&expected)
    );
    drop(reopened);
    assert_eq!(
        SqliteIndex::open(&path)
            .unwrap()
            .pending_rows()
            .await
            .unwrap(),
        [expected]
    );
}

#[tokio::test]
async fn layout_8_provenance_is_kept_and_read_as_built_from_local_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = SqliteIndex::open(&path).unwrap();
    let row = PendingRow {
        container_id: ContainerId::from_bytes([8; 16]),
        spool_path: dir.path().join("spool"),
        batch: BatchId::new("interrupted"),
        created_at: DeviceTime::from_unix_seconds(1),
        state: SpoolState::Spooled(None),
        commit_attempted: false,
        materializes: false,
    };
    index.record_pending_row(row.clone()).await.unwrap();
    drop(index);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "ALTER TABLE pending_rows DROP COLUMN materializes; \
             ALTER TABLE local_entries DROP COLUMN hash; DROP TABLE held_parcels; \
             DROP TABLE attempted_slots; ALTER TABLE entries DROP COLUMN captured_path; \
             ALTER TABLE local_entries DROP COLUMN relocate_to; PRAGMA user_version = 8;",
        )
        .unwrap();
    drop(connection);
    // The commit-attempt marker layout 8 already kept stays what it was; the
    // marker it lacked reads as the only kind of row that layout could hold
    // (spec: OC-2, OC-7).
    let expected = PendingRow {
        materializes: true,
        ..row
    };
    assert_eq!(
        SqliteIndex::open(&path)
            .unwrap()
            .pending_rows()
            .await
            .unwrap(),
        [expected]
    );
}

/// Layout 9 recorded no content hash beside a materialization. Its rows are
/// kept, and read back with none, which is what makes a scan keep such a file
/// rather than guess once it changed (spec: EP-15).
#[tokio::test]
async fn layout_9_materializations_are_kept_without_a_hash() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = SqliteIndex::open(&path).unwrap();
    let observation = LocalObservation {
        path: EntryPath::parse("albums/a.jpg").unwrap(),
        size: 100,
        mtime: Mtime::from_unix_seconds(1_700_000_000),
        at: DeviceTime::from_unix_seconds(1_700_000_400),
        hash: Some(ContentHash::from_bytes([0x3c; ContentHash::BYTE_LEN])),
    };
    index.mark_present(observation.clone()).await.unwrap();
    drop(index);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "ALTER TABLE local_entries DROP COLUMN hash; DROP TABLE held_parcels; \
             DROP TABLE attempted_slots; ALTER TABLE entries DROP COLUMN captured_path; \
             ALTER TABLE local_entries DROP COLUMN relocate_to; PRAGMA user_version = 9;",
        )
        .unwrap();
    drop(connection);

    let reopened = SqliteIndex::open(&path).unwrap();
    let row = reopened
        .local_entry_at(&observation.path)
        .await
        .unwrap()
        .expect("the materialization survives the upgrade");
    assert_eq!(row.state, LocalEntryState::Present);
    assert_eq!(
        row.observation,
        LocalObservation {
            hash: None,
            ..observation
        }
    );
}

/// Layout 10 kept no parcel. Its device state is kept whole, and it reopens
/// holding none — a device that has read nothing it still keeps — and able to
/// hold one from then on (spec: PK-21).
#[tokio::test]
async fn layout_10_device_state_is_kept_and_holds_no_parcel() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = SqliteIndex::open(&path).unwrap();
    let observation = LocalObservation {
        path: EntryPath::parse("books/atlas/003.jpg").unwrap(),
        size: 100,
        mtime: Mtime::from_unix_seconds(1_700_000_000),
        at: DeviceTime::from_unix_seconds(1_700_000_400),
        hash: Some(ContentHash::from_bytes([0x3c; ContentHash::BYTE_LEN])),
    };
    index.mark_present(observation.clone()).await.unwrap();
    drop(index);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "DROP TABLE held_parcels; DROP TABLE attempted_slots; \
             ALTER TABLE entries DROP COLUMN captured_path; ALTER TABLE local_entries DROP COLUMN relocate_to; PRAGMA user_version = 10;",
        )
        .unwrap();
    drop(connection);

    let reopened = SqliteIndex::open(&path).unwrap();
    let row = reopened
        .local_entry_at(&observation.path)
        .await
        .unwrap()
        .expect("the materialization survives the upgrade");
    assert_eq!(row.observation, observation);
    assert!(reopened.held_parcels().await.unwrap().is_empty());

    let parcel = HeldParcel {
        container_id: ContainerId::from_bytes([7; 16]),
        index: 2,
        plaintext: 2048..3072,
        path: dir.path().join("parcels").join("held"),
    };
    reopened.hold_parcel(parcel.clone()).await.unwrap();
    drop(reopened);
    assert_eq!(
        SqliteIndex::open(&path)
            .unwrap()
            .held_parcels()
            .await
            .unwrap(),
        [parcel]
    );
}

#[tokio::test]
async fn an_alias_of_the_same_index_cannot_bypass_ownership() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let alias = dir.path().join("alias.sqlite");
    let first = SqliteIndex::open(&path).unwrap();
    std::os::unix::fs::symlink(&path, &alias).unwrap();
    let second = SqliteIndex::open(&alias).unwrap();
    let _owner = first.own_pending_rows().await.unwrap();
    assert!(matches!(
        second.own_pending_rows().await,
        Err(IndexError::PendingRowsBusy { .. })
    ));
}

#[tokio::test]
async fn distinct_index_files_do_not_share_pending_ownership() {
    let dir = tempfile::tempdir().unwrap();
    let first = SqliteIndex::open(dir.path().join("index.sqlite")).unwrap();
    let second = SqliteIndex::open(dir.path().join("index.db")).unwrap();
    let _first_owner = first.own_pending_rows().await.unwrap();
    let _second_owner = second.own_pending_rows().await.unwrap();
}

/// Layout 11 recorded no commit slot. An attempted row it holds is kept as it
/// was and reads back with no slot for its batch, which is the missing evidence
/// a settle retains it for (spec: OC-2, OC-3); and the batch records slots from
/// then on.
#[tokio::test]
async fn layout_11_attempts_are_kept_with_no_slot() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = SqliteIndex::open(&path).unwrap();
    let row = PendingRow {
        container_id: ContainerId::from_bytes([11; 16]),
        spool_path: dir.path().join("spool"),
        batch: BatchId::new("interrupted"),
        created_at: DeviceTime::from_unix_seconds(1),
        state: SpoolState::Spooled(Some(ObjectRef::new("uploaded"))),
        commit_attempted: true,
        materializes: true,
    };
    index.record_pending_row(row.clone()).await.unwrap();
    drop(index);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "DROP TABLE attempted_slots; ALTER TABLE entries DROP COLUMN captured_path; \
             ALTER TABLE local_entries DROP COLUMN relocate_to; PRAGMA user_version = 11;",
        )
        .unwrap();
    drop(connection);

    let reopened = SqliteIndex::open(&path).unwrap();
    assert_eq!(
        reopened.pending_rows().await.unwrap(),
        std::slice::from_ref(&row)
    );
    assert!(reopened
        .attempted_slots(&row.batch)
        .await
        .unwrap()
        .is_empty());

    let slot = AttemptedSlot::new(Generation::new(3).unwrap(), Some("minted".to_owned()));
    reopened
        .mark_commit_attempted(&[row.container_id], &slot)
        .await
        .unwrap();
    drop(reopened);
    assert_eq!(
        SqliteIndex::open(&path)
            .unwrap()
            .attempted_slots(&row.batch)
            .await
            .unwrap(),
        [slot]
    );
}

/// A batch whose marking fails partway is left with no row marked and no slot
/// recorded (spec: OC-2, OC-3).
///
/// A trigger refuses the second row's update, so the first row's mark and its
/// slot have already been written inside the transaction when the write fails.
/// What a later settle reads must still be one answer for the whole batch:
/// every row unmarked and no slot, which is true, since a commit whose marking
/// failed sends no create.
#[tokio::test]
async fn a_batch_marked_attempted_partway_is_left_unmarked() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = SqliteIndex::open(&path).unwrap();
    let row = |seed: u8| PendingRow {
        container_id: ContainerId::from_bytes([seed; 16]),
        spool_path: dir.path().join(format!("spool-{seed}")),
        batch: BatchId::new("one-batch"),
        created_at: DeviceTime::from_unix_seconds(1),
        state: SpoolState::Spooled(Some(ObjectRef::new(format!("uploaded-{seed}")))),
        commit_attempted: false,
        materializes: true,
    };
    let rows = [row(1), row(2), row(3)];
    for row in &rows {
        index.record_pending_row(row.clone()).await.unwrap();
    }
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER refuse_the_second_mark BEFORE UPDATE OF commit_attempted \
             ON pending_rows WHEN old.container_id = X'02020202020202020202020202020202' \
             BEGIN SELECT RAISE(ABORT, 'interrupted partway through the batch'); END;",
        )
        .unwrap();

    let ids: Vec<_> = rows.iter().map(|row| row.container_id).collect();
    let slot = AttemptedSlot::new(Generation::new(5).unwrap(), None);
    assert!(matches!(
        index.mark_commit_attempted(&ids, &slot).await,
        Err(IndexError::Backend { .. })
    ));
    assert_eq!(
        index.pending_rows().await.unwrap(),
        rows,
        "the first row's mark must be rolled back with the rest"
    );
    assert!(
        index
            .attempted_slots(&BatchId::new("one-batch"))
            .await
            .unwrap()
            .is_empty(),
        "the slot recorded beside the first row's mark must be rolled back with it"
    );
}

/// A slot recorded without the rows it was for is rolled back too (spec: OC-2).
///
/// Here the trigger refuses the slot rather than a mark, after every row has
/// been marked: the rows must come back unmarked, since a marked row whose slot
/// is missing could never be proven not to have committed (spec: OC-3).
#[tokio::test]
async fn a_batch_whose_slot_cannot_be_recorded_is_left_unmarked() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = SqliteIndex::open(&path).unwrap();
    let row = PendingRow {
        container_id: ContainerId::from_bytes([4; 16]),
        spool_path: dir.path().join("spool"),
        batch: BatchId::new("one-batch"),
        created_at: DeviceTime::from_unix_seconds(1),
        state: SpoolState::Spooled(Some(ObjectRef::new("uploaded"))),
        commit_attempted: false,
        materializes: true,
    };
    index.record_pending_row(row.clone()).await.unwrap();
    rusqlite::Connection::open(&path)
        .unwrap()
        .execute_batch(
            "CREATE TRIGGER refuse_the_slot BEFORE INSERT ON attempted_slots \
             BEGIN SELECT RAISE(ABORT, 'interrupted before the slot was recorded'); END;",
        )
        .unwrap();

    let slot = AttemptedSlot::new(Generation::new(5).unwrap(), None);
    assert!(matches!(
        index
            .mark_commit_attempted(&[row.container_id], &slot)
            .await,
        Err(IndexError::Backend { .. })
    ));
    assert_eq!(index.pending_rows().await.unwrap(), [row]);
    assert!(index
        .attempted_slots(&BatchId::new("one-batch"))
        .await
        .unwrap()
        .is_empty());
}

/// Layout 12's device state is upgraded whole, and its catalog is discarded
/// rather than migrated: it was filled from Storage objects in a format this
/// build no longer reads, so the next catch-up rebuilds it from Storage
/// (spec: RV-5, EP-10, OC-2). The catalog laid out in its place is the one a
/// fresh file lays out.
#[tokio::test]
async fn a_layout_12_file_keeps_its_device_state_and_loses_its_catalog() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    let index = SqliteIndex::open(&path).unwrap();
    index.restore(snapshot(4)).await.unwrap();
    let observation = LocalObservation {
        path: EntryPath::parse("albums/1.jpg").unwrap(),
        size: 100,
        mtime: Mtime::from_unix_seconds(1_700_000_000),
        at: DeviceTime::from_unix_seconds(1_700_000_400),
        hash: Some(ContentHash::from_bytes([0x3c; ContentHash::BYTE_LEN])),
    };
    index.mark_present(observation.clone()).await.unwrap();
    let row = PendingRow {
        container_id: ContainerId::from_bytes([12; 16]),
        spool_path: dir.path().join("spool"),
        batch: BatchId::new("interrupted"),
        created_at: DeviceTime::from_unix_seconds(1),
        state: SpoolState::Spooled(Some(ObjectRef::new("uploaded"))),
        commit_attempted: true,
        materializes: true,
    };
    index.record_pending_row(row.clone()).await.unwrap();
    drop(index);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch("ALTER TABLE entries DROP COLUMN captured_path; ALTER TABLE local_entries DROP COLUMN relocate_to; PRAGMA user_version = 12;")
        .unwrap();
    drop(connection);

    let reopened = SqliteIndex::open(&path).unwrap();
    assert_eq!(
        stamp_of(&path),
        16,
        "the file is upgraded to the current layout"
    );
    assert!(
        reopened.checkpoint().await.unwrap().is_none(),
        "the catalog is discarded, to be caught up again from Storage"
    );
    assert_eq!(
        reopened
            .local_entry_at(&observation.path)
            .await
            .unwrap()
            .expect("the materialization survives the upgrade")
            .observation,
        observation,
    );
    assert_eq!(reopened.pending_rows().await.unwrap(), [row]);
    drop(reopened);

    let fresh = dir.path().join("fresh.sqlite");
    drop(SqliteIndex::open(&fresh).unwrap());
    assert_eq!(entries_table(&path), entries_table(&fresh));
}

/// How a file declares its table of Entries and the index beside it.
fn entries_table(file: &std::path::Path) -> Vec<String> {
    let connection = rusqlite::Connection::open(file).unwrap();
    let mut statement = connection
        .prepare("SELECT sql FROM sqlite_schema WHERE tbl_name = 'entries' AND sql IS NOT NULL ORDER BY name")
        .unwrap();
    statement
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}
