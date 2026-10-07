use coffret_model::{ContainerId, ObjectRef};
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::device_state::{BatchId, DeviceTime, PendingRow, SpoolState};
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
    };
    index.record_pending_row(row.clone()).await.unwrap();
    drop(index);
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            "ALTER TABLE pending_rows DROP COLUMN commit_attempted; PRAGMA user_version = 7;",
        )
        .unwrap();
    drop(connection);
    let reopened = SqliteIndex::open(&path).unwrap();
    let expected = PendingRow {
        commit_attempted: true,
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
