use coffret_model::{ContainerId, ContentHash, EntryPath, Mtime, ObjectRef};
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::device_state::{
    BatchId, DeviceTime, LocalEntryState, LocalObservation, PendingRow, SpoolState,
};
use coffret_usecase::{Index, IndexError};

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
             ALTER TABLE local_entries DROP COLUMN hash; PRAGMA user_version = 7;",
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
             ALTER TABLE local_entries DROP COLUMN hash; PRAGMA user_version = 8;",
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
        .execute_batch("ALTER TABLE local_entries DROP COLUMN hash; PRAGMA user_version = 9;")
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
