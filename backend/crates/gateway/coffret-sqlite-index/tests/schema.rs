//! What opening an Index file does, and refuses to do.
//!
//! The catalog can be rebuilt exactly from Storage (spec: RV-5), which is what
//! makes refusing a file this build does not understand the right answer rather
//! than a harsh one: there is nothing in it that is not somewhere else, and a
//! build guessing at a layout it never wrote is how a cache starts giving wrong
//! answers.

use coffret_model::{ContainerKind, ContainerSummary, ContentHash, Generation, ObjectRef};
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::{Index, IndexError};

mod support;

use support::{checkpoint, container_id, snapshot};

/// A path inside a directory that lives as long as the case.
struct Scratch {
    directory: tempfile::TempDir,
}

impl Scratch {
    fn new() -> Self {
        Self {
            directory: tempfile::tempdir().expect("a temporary directory must be creatable"),
        }
    }

    fn file(&self) -> std::path::PathBuf {
        self.directory.path().join("index.sqlite")
    }
}

/// A fresh file gets the layout, and keeps what is written into it.
#[tokio::test]
async fn a_fresh_file_becomes_a_catalog() {
    let scratch = Scratch::new();

    let index = SqliteIndex::open(scratch.file()).expect("a fresh file must open as a catalog");
    assert!(
        index
            .checkpoint()
            .await
            .expect("reading a fresh checkpoint must succeed")
            .is_none(),
        "a fresh catalog stands at no committed state"
    );

    index
        .restore(snapshot(4))
        .await
        .expect("restoring a Snapshot must succeed");
    assert_eq!(
        index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed"),
        Some(checkpoint(4))
    );
}

/// Reopening a file finds the catalog that was left in it.
#[tokio::test]
async fn an_existing_file_reopens() {
    let scratch = Scratch::new();

    {
        let index = SqliteIndex::open(scratch.file()).expect("a fresh file must open");
        index
            .restore(snapshot(4))
            .await
            .expect("restoring a Snapshot must succeed");
    }

    let reopened = SqliteIndex::open(scratch.file()).expect("an existing file must reopen");
    let content = reopened
        .snapshot()
        .await
        .expect("the catalog left in the file has a state to checkpoint");
    assert_eq!(content, snapshot(4));
    assert_eq!(
        content.containers,
        [ContainerSummary {
            id: container_id(1),
            kind: ContainerKind::Pack,
            ciphertext_hash: ContentHash::from_bytes([1; ContentHash::BYTE_LEN]),
            ciphertext_len: 164,
            object_ref: Some(ObjectRef::new("stored-1")),
        }]
    );
}

/// A file stamped with a layout this build does not write is refused.
#[tokio::test]
async fn a_file_from_another_layout_is_refused() {
    let scratch = Scratch::new();

    {
        let connection =
            rusqlite::Connection::open(scratch.file()).expect("a fresh file must open");
        connection
            .pragma_update(None, "user_version", 99)
            .expect("stamping a version must succeed");
    }

    let result = SqliteIndex::open(scratch.file());
    assert!(
        matches!(
            result.as_ref().err(),
            Some(IndexError::UnsupportedSchema {
                found: 99,
                supported: 1,
            })
        ),
        "expected a layout this build does not know to be refused, got {:?}",
        result.err()
    );
}

/// A catalog that has only replayed records has adopted no Snapshot, and a
/// replay does not disturb the record of one that was adopted (spec: CK-9).
#[tokio::test]
async fn a_replay_leaves_the_adopted_snapshot_as_it_was() {
    let scratch = Scratch::new();
    let index = SqliteIndex::open(scratch.file()).expect("a fresh file must open");

    index
        .apply(support::record(0))
        .await
        .expect("replaying a record must succeed");
    assert_eq!(
        index
            .snapshot()
            .await
            .expect("an applied catalog has a state to checkpoint")
            .adopted_from,
        None,
        "a catalog that has only replayed records adopted nothing"
    );

    index
        .restore(snapshot(4))
        .await
        .expect("restoring a Snapshot must succeed");
    index
        .apply(support::record(5))
        .await
        .expect("replaying a record must succeed");

    let content = index
        .snapshot()
        .await
        .expect("a caught-up catalog has a state to checkpoint");
    assert_eq!(
        content.adopted_from,
        Some(coffret_model::ControlObjectName::index_snapshot(
            Generation::new(4)
        )),
        "the Snapshot this catalog started from is still what it started from"
    );
    assert_eq!(content.checkpoint.head_generation, Generation::new(5));
    assert_eq!(
        content.checkpoint.next_commit_slot.as_deref(),
        Some("minted-5"),
        "the slot the head carries survives the file (spec: CP-2)"
    );
}
