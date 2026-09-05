//! What the catalog does with a number outside the range its writers produce.
//!
//! The values a catalog keeps are unsigned in the domain and signed in the
//! file, and the two halves of that mismatch are two different situations. A
//! value at or past `2^63` is one this catalog has no column for, and it
//! arrives from a caller — so it is refused on the way in, before it reaches a
//! column. Every number the format carries is already below that bound by the
//! time it gets here (spec: FM-19), so the one value that can still reach the
//! refusal is a length this device's own filesystem reported. A negative value
//! can only be read, never written, so finding one says the file was written by
//! something that is not this build, or damaged since: the verdict a malformed
//! path in the same row gets, and for the same reason — the catalog is a cache
//! that Storage can rebuild (spec: RV-5).
//!
//! The third case is what a refusal costs: a write that is refused partway
//! leaves the catalog exactly as it found it.

use std::path::Path;

use coffret_model::Mtime;
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::device_state::{DeviceTime, LocalObservation};
use coffret_usecase::{Index, IndexError};

mod support;

use support::{entry_path, rows_in, snapshot, Scratch};

/// The first size a signed 64-bit column has no spelling for.
const PAST_THE_RANGE: u64 = 1 << 63;

/// A file holding the catalog `snapshot(4)` describes, closed again.
///
/// Closed, because every case below then opens the same file a second time —
/// once through `rusqlite` to write something the adapter never would, and once
/// more through the port to see what it makes of it.
async fn a_file_with_a_catalog(scratch: &Scratch) {
    let index = SqliteIndex::open(scratch.file()).expect("a fresh file must open");
    index
        .restore(snapshot(4))
        .await
        .expect("restoring a Snapshot must succeed");
}

/// Runs one statement against the file, behind the adapter's back.
///
/// The adapter writes no negative integer into any of these columns, so a file
/// holding one has to be made this way — the same way every case in this crate
/// that needs a file no build of this adapter wrote makes one.
fn poke(file: &Path, statement: &str) {
    rusqlite::Connection::open(file)
        .expect("the Index file must open")
        .execute(statement, [])
        .expect("a statement against this build's own layout must run");
}

/// What this device would be recording if its filesystem reported a file of
/// `2^63` bytes.
///
/// The size is the device's own observation rather than anything the format
/// carried, which is exactly why it is the value that can still reach the
/// refusal: nothing bounded it on the way in (spec: EP-10, FM-19).
fn an_observation_past_the_range() -> LocalObservation {
    LocalObservation {
        path: entry_path("albums/1.jpg"),
        size: PAST_THE_RANGE,
        mtime: Mtime::from_unix_seconds(1_700_000_000),
        at: DeviceTime::from_unix_seconds(1_700_000_400),
    }
}

/// A negative integer is read as a file this build cannot read, whichever of
/// the two groups of columns holds it, and the report names the column.
///
/// Two columns rather than one because they are reached by different readers:
/// an Entry's `offset` comes back through a row of `entries`, and
/// `head_generation` through the single row of `checkpoint`. Naming the column
/// is what tells whoever is holding the file which of a row's numbers was the
/// one nothing could have written.
#[tokio::test]
async fn a_negative_integer_in_a_catalog_column_makes_the_catalog_unreadable() {
    for (statement, column) in [
        ("UPDATE entries SET \"offset\" = -1", "offset"),
        (
            "UPDATE checkpoint SET head_generation = -1",
            "head_generation",
        ),
    ] {
        let scratch = Scratch::new();
        a_file_with_a_catalog(&scratch).await;
        poke(&scratch.file(), statement);

        let index = SqliteIndex::open(scratch.file()).expect("the file must still open");
        let refused = index.snapshot().await;

        assert!(
            matches!(refused, Err(IndexError::UnreadableCatalog { .. })),
            "expected {column} to make the catalog unreadable, got {refused:?}",
        );
        let reported = refused
            .expect_err("the case just asserted a refusal")
            .to_string();
        assert!(reported.contains(column), "{reported}");
    }
}

/// A value the catalog has no column for is refused on the way in, under the
/// name of what it is: not a catalog this build cannot read, and not the store
/// failing.
///
/// The value is a device-observed size because that is the only one left that
/// can get this far: every number the format carries is refused at its own
/// constructor before a catalog ever sees it (spec: FM-19).
#[tokio::test]
async fn an_observed_size_past_the_catalogs_integer_range_is_refused_on_write() {
    let scratch = Scratch::new();
    let index = SqliteIndex::open(scratch.file()).expect("a fresh file must open");

    let refused = index.mark_present(an_observation_past_the_range()).await;

    assert!(
        matches!(
            refused,
            Err(IndexError::UnrepresentableValue {
                column: "observed_size",
                value: PAST_THE_RANGE,
                ..
            })
        ),
        "expected the observed size to be refused, got {refused:?}",
    );
}

/// The refusal above leaves the catalog it was written against exactly as it
/// was — every Container, every Entry, the checkpoint the file stood at, and no
/// row for the file the device could not record.
#[tokio::test]
async fn a_refused_write_leaves_the_catalog_as_it_was() {
    let scratch = Scratch::new();
    {
        let index = SqliteIndex::open(scratch.file()).expect("a fresh file must open");
        index
            .restore(snapshot(4))
            .await
            .expect("restoring a Snapshot must succeed");
        let before = index.snapshot().await.expect("the catalog must read back");

        index
            .mark_present(an_observation_past_the_range())
            .await
            .expect_err("a size past the column's range must be refused");

        let after = index
            .snapshot()
            .await
            .expect("the catalog must still read back");
        assert_eq!(after, before);
    }

    assert_eq!(rows_in(&scratch.file(), "containers"), 1);
    assert_eq!(rows_in(&scratch.file(), "entries"), 2);
    assert_eq!(rows_in(&scratch.file(), "local_entries"), 0);
}
