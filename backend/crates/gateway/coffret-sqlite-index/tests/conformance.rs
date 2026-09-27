//! The Index port's contract, run against a real SQLite file.
//!
//! The same suite the in-memory catalog runs, so that the two cannot quietly
//! mean different things by the port: a device that replays records into a file
//! and one that replays them into memory have to land on the same catalog, or a
//! Snapshot written from either is not the Library's state.
//!
//! It needs nothing but a temporary directory, so it is part of an ordinary
//! `cargo test`.

use std::path::PathBuf;

use coffret_model::ObjectRef;
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::device_state::PendingRow;
use coffret_usecase::index_conformance::{IndexUnderTest, StoredForm};

/// The file behind the catalog a case drives, written through `rusqlite`
/// directly for what the adapter never writes.
struct File(PathBuf);

impl StoredForm for File {
    fn plant_spooling_row_with_object(&self, row: &PendingRow, object: &ObjectRef) {
        let connection = rusqlite::Connection::open(&self.0).expect("the Index file must open");
        connection
            .execute(
                "INSERT INTO pending_rows \
                     (container_id, spool_path, state, batch, created_at, object_ref) \
                 VALUES (?1, ?2, 'spooling', ?3, ?4, ?5)",
                rusqlite::params![
                    row.container_id.as_bytes().as_slice(),
                    row.spool_path.to_str().expect("a fixture path is UTF-8"),
                    row.batch.as_str(),
                    row.created_at.as_unix_seconds(),
                    object.as_str(),
                ],
            )
            .expect("planting a row must succeed");
    }
}

/// Two catalogs in two files, in a directory that goes away with the case.
///
/// Two files rather than two connections to one: the suite's comparisons are
/// between independent devices' catalogs, and sharing a file would let one
/// case's writes answer the other's reads.
async fn fixture() -> Option<IndexUnderTest> {
    let directory = tempfile::tempdir().expect("a temporary directory must be creatable");
    let file = directory.path().join("index.sqlite");
    let index = SqliteIndex::open(&file).expect("opening a fresh Index file must succeed");
    let other = SqliteIndex::open(directory.path().join("other.sqlite"))
        .expect("opening a second fresh Index file must succeed");

    Some(
        IndexUnderTest::new(Box::new(index), Box::new(other))
            .with_stored_form(Box::new(File(file)))
            .holding(Box::new(directory)),
    )
}

coffret_usecase::index_conformance!(fixture().await);
