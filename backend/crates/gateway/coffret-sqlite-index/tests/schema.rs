//! What opening an Index file does with the layout it finds.
//!
//! One file holds two groups of tables and only one of them is a cache. The
//! catalog can be rebuilt exactly from Storage (spec: RV-5), so a catalog this
//! build does not understand is thrown away rather than converted or clung to.
//! What sits beside it cannot be: where this device maps the Library, what it
//! has on disk, and the spools it has not committed are written down nowhere
//! else (spec: EP-9, EP-10, OC-2), and no catch-up brings any of it back.
//!
//! So an older file is opened for as long as that second group is one this
//! build reads, and refused once it is not. These cases are about where that
//! line falls, what survives on each side of it, and what a build guessing at a
//! layout it never wrote is stopped from doing.

use std::path::{Path, PathBuf};

use coffret_logging::testing::CapturedLogs;
use coffret_model::{
    ContainerKind, ContainerSummary, ContentHash, EntryPath, Generation, Mtime, ObjectRef,
};
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::device_state::{
    BatchId, DeviceTime, LocalEntry, LocalEntryState, LocalObservation, Mapping, PendingUpload,
    RootIdentity, SpoolState,
};
use coffret_usecase::{Index, IndexError};
use tracing::Level;

mod support;

use support::{checkpoint, container_id, rows_in, snapshot, stamp_of, Scratch};

/// The layout this build writes, and the one its device-local group last
/// changed at.
///
/// Written out rather than read from the adapter, which keeps them to itself.
/// A case that moved with the constant would stop being a case about these two
/// numbers, and it is the numbers — an older catalog beside a device group this
/// build still reads — that decide everything below.
const SCHEMA_VERSION: i64 = 5;
const DEVICE_SCHEMA_VERSION: i64 = 4;

/// Where one part of the Library lives on this device (spec: EP-9).
fn mapping() -> Mapping {
    Mapping {
        prefix: Some(EntryPath::nfc("albums")),
        local_root: PathBuf::from("/somewhere/albums"),
        // Stamped, as a scan that has seen the root leaves it (spec: EP-12):
        // the column a discard must not quietly clear.
        root_identity: Some(RootIdentity::new("volume-7")),
    }
}

/// One file this device has materialized (spec: EP-10).
fn observation() -> LocalObservation {
    LocalObservation {
        path: EntryPath::nfc("albums/1.jpg"),
        size: 100,
        mtime: Mtime::from_unix_seconds(1_700_000_000),
        at: DeviceTime::from_unix_seconds(1_700_000_400),
    }
}

/// One pending row an interrupted run left behind, which nothing outside this
/// file records (spec: OC-2, OC-7).
fn pending() -> PendingUpload {
    PendingUpload {
        container_id: container_id(9),
        spool_path: PathBuf::from("/somewhere/spool/9.pack"),
        state: SpoolState::Spooled,
        batch: BatchId::new("batch-9"),
        created_at: DeviceTime::from_unix_seconds(1_700_000_500),
        object_ref: Some(ObjectRef::new("stored-9")),
    }
}

/// A file holding a whole catalog and this device's own state, stamped as the
/// layout `version` was.
///
/// Everything goes in through the port and the stamp is rewritten afterwards.
/// Within the range this build still reads, an older layout differs from the
/// current one in the catalog alone, so the current DDL is the right shape for
/// both groups; below it the stamp alone decides, before any table is looked
/// at. Either way the stamp is the whole of what makes the file an older one.
async fn a_file_stamped(scratch: &Scratch, version: i64) {
    {
        let index = SqliteIndex::open(scratch.file()).expect("a fresh file must open");
        index
            .restore(snapshot(4))
            .await
            .expect("restoring a Snapshot must succeed");
        index
            .set_mapping(mapping())
            .await
            .expect("recording a mapping must succeed");
        index
            .mark_present(observation())
            .await
            .expect("recording a materialized file must succeed");
        index
            .record_pending_upload(pending())
            .await
            .expect("recording a spool must succeed");
    }
    restamp(&scratch.file(), version);
}

/// Stamps a layout version into a file, behind the adapter's back.
fn restamp(file: &Path, version: i64) {
    rusqlite::Connection::open(file)
        .expect("the Index file must open")
        .pragma_update(None, "user_version", version)
        .expect("stamping a version must succeed");
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

/// An older catalog is thrown away; the device's own state is not.
#[tokio::test]
async fn an_older_library_layout_is_discarded_and_the_device_state_kept() {
    let scratch = Scratch::new();
    a_file_stamped(&scratch, DEVICE_SCHEMA_VERSION).await;

    let index = SqliteIndex::open(scratch.file())
        .expect("a layout whose device state this build reads must open");

    assert_eq!(
        index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed"),
        None,
        "the discarded catalog stands where a fresh one does: at no committed state"
    );
    assert!(
        index
            .entries_under(None)
            .await
            .expect("listing a fresh catalog must succeed")
            .is_empty(),
        "nothing of the old catalog is left to be read back"
    );

    // And every row of the group no Snapshot carries, unchanged.
    assert_eq!(
        index
            .mappings()
            .await
            .expect("reading the mappings must succeed"),
        vec![mapping()],
        "the mapping is the one record of where the Library lives on this device"
    );
    assert_eq!(
        index
            .local_entry_at(&observation().path)
            .await
            .expect("reading a local file's row must succeed"),
        Some(LocalEntry {
            observation: observation(),
            state: LocalEntryState::Present,
        })
    );
    assert_eq!(
        index
            .pending_uploads()
            .await
            .expect("reading the spools must succeed"),
        vec![pending()],
        "the spool row is the only thing that knows an interrupted run wrote a file"
    );

    drop(index);
    assert_eq!(
        stamp_of(&scratch.file()),
        SCHEMA_VERSION,
        "the file is this build's now, and the next open has nothing left to discard"
    );
}

/// What a discard leaves behind is caught up with the way anything else is:
/// there is no repair path of its own, because there is nothing special about
/// an emptied catalog.
#[tokio::test]
async fn a_discarded_catalog_is_rebuilt_by_the_next_catch_up() {
    let older = Scratch::new();
    a_file_stamped(&older, DEVICE_SCHEMA_VERSION).await;
    let rebuilt = SqliteIndex::open(older.file()).expect("an older layout must open");

    let fresh_file = Scratch::new();
    let fresh = SqliteIndex::open(fresh_file.file()).expect("a fresh file must open");

    for index in [&rebuilt, &fresh] {
        index
            .restore(snapshot(4))
            .await
            .expect("restoring a Snapshot must succeed");
        index
            .apply(support::record(5))
            .await
            .expect("replaying a record must succeed");
    }

    assert_eq!(
        rebuilt
            .snapshot()
            .await
            .expect("a caught-up catalog has a state to checkpoint"),
        fresh
            .snapshot()
            .await
            .expect("a caught-up catalog has a state to checkpoint"),
        "the same calls reach the same catalog, whatever the file held before"
    );
    assert_eq!(
        rebuilt
            .mappings()
            .await
            .expect("reading the mappings must succeed"),
        vec![mapping()],
        "and the catch-up left this device's own state where the discard did"
    );
}

/// A layout whose device-local group this build cannot read is refused, and
/// refusing leaves the file alone.
#[tokio::test]
async fn a_layout_older_than_the_device_state_is_refused() {
    let scratch = Scratch::new();
    a_file_stamped(&scratch, DEVICE_SCHEMA_VERSION - 1).await;

    let result = SqliteIndex::open(scratch.file());
    assert!(
        matches!(
            result.as_ref().err(),
            Some(IndexError::UnsupportedSchema { found, supported })
                if *found == DEVICE_SCHEMA_VERSION - 1 && *supported == SCHEMA_VERSION
        ),
        "expected a device-local layout this build cannot read to be refused, got {:?}",
        result.err()
    );

    // A refusal is not half a discard: the owner is being told to save what is
    // in the file, and it has to still be in it.
    for (table, rows) in [
        ("checkpoint", 1),
        ("containers", 1),
        ("entries", 2),
        ("mappings", 1),
        ("local_entries", 1),
        ("pending_uploads", 1),
    ] {
        assert_eq!(
            rows_in(&scratch.file(), table),
            rows,
            "the refused file kept every row of {table}"
        );
    }
    assert_eq!(
        stamp_of(&scratch.file()),
        DEVICE_SCHEMA_VERSION - 1,
        "and it is still stamped as the layout it was written to"
    );
}

/// A file from a build that came after this one is refused too, and for the
/// plainer reason: none of it is readable here.
#[tokio::test]
async fn a_layout_from_a_newer_build_is_refused() {
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
                supported: SCHEMA_VERSION,
            })
        ),
        "expected a layout this build does not know to be refused, got {:?}",
        result.err()
    );
}

/// Two connections over one old file rebuild it once.
///
/// A server answering a browser while the same person runs a sync in a terminal
/// is two processes over one file, and both may reach an old one at the same
/// moment. The second must find the layout the first left and let it be — a
/// second discard would throw away the catalog the first one had begun to
/// rebuild.
#[tokio::test]
async fn a_second_connection_finds_the_rebuilt_layout() {
    let scratch = Scratch::new();
    a_file_stamped(&scratch, DEVICE_SCHEMA_VERSION).await;

    let first = SqliteIndex::open(scratch.file()).expect("an older layout must open");
    let second =
        SqliteIndex::open(scratch.file()).expect("a second connection must find a current layout");

    assert_eq!(
        stamp_of(&scratch.file()),
        SCHEMA_VERSION,
        "the layout the first open left is what the second one found"
    );
    assert_eq!(
        second
            .mappings()
            .await
            .expect("reading the mappings must succeed")
            .len(),
        1,
        "one mapping was recorded and one is there: neither open touched the group"
    );
    assert!(
        first
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed")
            .is_none(),
        "and both connections are looking at the same emptied catalog"
    );
}

/// The discard is recorded, and the record says nothing about where the file is.
///
/// The Index lives under the state directory and its path is the owner's own,
/// which is one of the things an event may never carry — so the two versions
/// and what was done with them are the whole of it.
#[tokio::test]
async fn the_discard_is_logged_without_a_path() {
    let scratch = Scratch::new();
    a_file_stamped(&scratch, DEVICE_SCHEMA_VERSION).await;

    let logs = CapturedLogs::capture();
    let _index = SqliteIndex::open(scratch.file()).expect("an older layout must open");

    let event = logs.only(Level::WARN);
    assert_eq!(event.number("found"), DEVICE_SCHEMA_VERSION);
    assert_eq!(event.number("supported"), SCHEMA_VERSION);
    assert!(
        event.field("operation").contains("discarding"),
        "the event says what was done: {event}"
    );
    logs.assert_free_of(&[
        scratch
            .path()
            .to_str()
            .expect("a temporary directory's path is UTF-8 here"),
        "index.sqlite",
    ]);
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

/// A path spelled with a combining acute rather than the composed character —
/// what no writer holding to EP-1 ever puts in a column.
const DECOMPOSED: &str = "cafe\u{301}.jpg";

/// Rewrites one text column of one table, behind the adapter's back.
///
/// The invariant makes a non-NFC path unbuildable through the port, which is
/// the point of it, so a file holding one is written at the SQL the adapter
/// itself would have used.
fn overwrite(file: &Path, statement: &str) {
    let connection = rusqlite::Connection::open(file).expect("the Index file must open");
    let changed = connection
        .execute(statement, [DECOMPOSED])
        .expect("the statement must run");
    assert_eq!(changed, 1, "the case rewrote exactly one row");
}

/// A stored Entry Path that is not NFC is a catalog this build cannot read
/// (spec: EP-1).
#[tokio::test]
async fn an_entry_path_that_is_not_in_nfc_is_unreadable() {
    let scratch = Scratch::new();

    {
        let index = SqliteIndex::open(scratch.file()).expect("a fresh file must open");
        index
            .restore(snapshot(4))
            .await
            .expect("restoring a Snapshot must succeed");
    }
    overwrite(
        &scratch.file(),
        "UPDATE entries SET path = ?1 WHERE path = (SELECT min(path) FROM entries)",
    );

    let index = SqliteIndex::open(scratch.file()).expect("an existing file must reopen");
    let result = index.entries_under(None).await;
    assert!(
        matches!(
            result.as_ref().err(),
            Some(IndexError::UnreadableCatalog {
                operation: "reading an Entry",
                ..
            })
        ),
        "expected a decomposed Entry Path to make the catalog unreadable, got {result:?}"
    );
}

/// The same of a mapping's prefix, which is device state rather than Library
/// state and read back through its own column (spec: EP-9).
#[tokio::test]
async fn a_mapping_prefix_that_is_not_in_nfc_is_unreadable() {
    let scratch = Scratch::new();

    {
        let index = SqliteIndex::open(scratch.file()).expect("a fresh file must open");
        index
            .set_mapping(Mapping {
                prefix: Some(EntryPath::nfc("albums")),
                local_root: PathBuf::from("/tmp/albums"),
                root_identity: None,
            })
            .await
            .expect("recording a mapping must succeed");
    }
    overwrite(&scratch.file(), "UPDATE mappings SET prefix = ?1");

    let index = SqliteIndex::open(scratch.file()).expect("an existing file must reopen");
    let result = index.mappings().await;
    assert!(
        matches!(
            result.as_ref().err(),
            Some(IndexError::UnreadableCatalog {
                operation: "reading a mapping",
                ..
            })
        ),
        "expected a decomposed mapping prefix to make the catalog unreadable, got {result:?}"
    );
}
