//! What the catalog does with a number outside the range its writers produce.
//!
//! Offsets, sizes, epochs, and generations are unsigned in the domain and
//! signed in the file, and the two halves of that mismatch are two different
//! situations. A value at or past `2^63` is one this catalog has no column for,
//! and it arrives from a caller — so it is refused on the way in, before it
//! reaches a column. A negative value can only be read, never written, so
//! finding one says the file was written by something that is not this build,
//! or damaged since: the verdict a malformed path in the same row gets, and for
//! the same reason — the catalog is a cache that Storage can rebuild
//! (spec: RV-5).
//!
//! The third case is what a refusal costs. A restore clears the catalog before
//! it fills it, so the one thing a refused write may not do is leave the file
//! somewhere between the two.

use std::path::Path;

use coffret_model::{
    CiphertextLenClaim, ContainerKind, ContainerSummary, ContentHash, EntryLocation, EntryMetadata,
    Mtime, ObjectRef,
};
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::{Index, IndexError, SnapshotContent};

mod support;

use support::{checkpoint, container_id, entry_path, extent, rows_in, snapshot, Scratch};

/// The first offset a signed 64-bit column has no spelling for.
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

/// A Snapshot whose one Entry begins past what the catalog can spell.
///
/// A zero-length Entry at that offset, so the extent itself is one the domain
/// admits — what is being asked about is the column, not [`EntryExtent`]'s own
/// rule about the address space.
///
/// [`EntryExtent`]: coffret_model::EntryExtent
fn a_snapshot_past_the_range() -> SnapshotContent {
    let id = container_id(1);
    SnapshotContent::canonical(
        checkpoint(4),
        None,
        vec![ContainerSummary {
            id,
            kind: ContainerKind::Pack,
            ciphertext_hash: ContentHash::from_bytes([1; ContentHash::BYTE_LEN]),
            ciphertext_len: CiphertextLenClaim::new(164),
            object_ref: Some(ObjectRef::new("stored-1")),
        }],
        vec![EntryLocation {
            container_id: id,
            entry: EntryMetadata {
                path: entry_path("albums/1.jpg"),
                extent: extent(PAST_THE_RANGE, 0),
                mtime: Mtime::from_unix_seconds(1_700_000_000),
                btime: None,
                hash: ContentHash::from_bytes([1; ContentHash::BYTE_LEN]),
                derived_from: None,
                mime: None,
            },
        }],
    )
    .expect("a fixture holds a Library an Index could stand at")
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
#[tokio::test]
async fn a_value_past_the_catalogs_integer_range_is_refused_on_write() {
    let scratch = Scratch::new();
    let index = SqliteIndex::open(scratch.file()).expect("a fresh file must open");

    let refused = index.restore(a_snapshot_past_the_range()).await;

    assert!(
        matches!(
            refused,
            Err(IndexError::UnrepresentableValue {
                column: "offset",
                value: PAST_THE_RANGE,
                ..
            })
        ),
        "expected the offset to be refused, got {refused:?}",
    );
}

/// The refusal above happens inside the transaction the restore runs in, so the
/// catalog it would have replaced is still there afterwards — every Container,
/// every Entry, and the checkpoint the file stood at.
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
            .restore(a_snapshot_past_the_range())
            .await
            .expect_err("a value past the column's range must be refused");

        let after = index
            .snapshot()
            .await
            .expect("the catalog must still read back");
        assert_eq!(after, before);
    }

    assert_eq!(rows_in(&scratch.file(), "containers"), 1);
    assert_eq!(rows_in(&scratch.file(), "entries"), 2);
}
