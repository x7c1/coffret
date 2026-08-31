//! One Index file, two connections to it.
//!
//! A server holding a Library open to answer a browser while the same person
//! runs a sync in a terminal is two processes over one catalog, and that is the
//! ordinary arrangement rather than a mistake. Under SQLite's default journal
//! mode every read on one connection would fail the moment the other one held a
//! write open, and the two would take turns breaking each other.
//!
//! Two connections in one test process is what that looks like from the file's
//! side: SQLite's locking is over the file rather than over the process, so a
//! second `Connection` here meets the first exactly as a second process would.

use std::path::Path;

use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::device_state::Mapping;
use coffret_usecase::Index;
use rusqlite::Connection;

// The catalog is readable while another connection is part-way through a write,
// which is the whole of what a shared Index file has to give — and it is given
// by the journal mode, so the mode itself is asserted beside it: without it this
// case would pass on the accident that a rollback-journal writer only excludes
// readers for the instant it commits.
#[tokio::test]
async fn a_read_answers_while_another_connection_holds_a_write_open() {
    let directory = tempfile::tempdir().expect("a temporary directory must be available");
    let file = directory.path().join("index.sqlite");

    let index = SqliteIndex::open(&file).expect("a new catalog is created where there is none");
    assert_eq!(journal_mode(&file), "wal");

    // The other process, mid-sync: its transaction has written and has not
    // committed.
    let writer = Connection::open(&file).expect("a second connection opens the same file");
    writer
        .execute_batch(
            "BEGIN IMMEDIATE; \
             INSERT INTO mappings (prefix, local_root, root_identity) \
             VALUES ('albums', '/somewhere/albums', NULL);",
        )
        .expect("the other connection takes the write lock");

    // Nothing here is allowed to wait for that transaction, let alone fail.
    let checkpoint = index
        .checkpoint()
        .await
        .expect("a read answers while another connection is writing");
    assert!(checkpoint.is_none(), "nothing has been committed yet");
    assert!(
        index
            .mappings()
            .await
            .expect("a listing answers too")
            .is_empty(),
        "an uncommitted write is not visible to the other connection",
    );

    writer
        .execute_batch("COMMIT")
        .expect("the other connection commits");

    // And what it committed is there afterwards, from the connection that was
    // reading throughout: one file, one catalog.
    let seen = index.mappings().await.expect("the catalog is readable");
    assert_eq!(seen.len(), 1);
    assert_eq!(
        seen[0].prefix.as_ref().map(|prefix| prefix.as_str()),
        Some("albums"),
    );

    // A write of this connection's own, now that the other one has let go.
    index
        .set_mapping(Mapping {
            prefix: None,
            local_root: directory.path().to_path_buf(),
            root_identity: None,
        })
        .await
        .expect("a write succeeds once the other writer has committed");
}

/// The journal mode the file itself is in, read through a connection of its own.
///
/// Write-ahead logging is recorded in the file rather than in a connection, so
/// this is the same answer whichever process asks — which is exactly the claim
/// worth checking.
fn journal_mode(file: &Path) -> String {
    Connection::open(file)
        .expect("the file opens")
        .query_row("PRAGMA journal_mode", [], |row| row.get::<_, String>(0))
        .expect("SQLite always answers with a mode")
}
