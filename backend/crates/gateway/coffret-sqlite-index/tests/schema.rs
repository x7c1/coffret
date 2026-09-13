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
//!
//! In *this* build the line falls at the current layout itself. The change that
//! gave a mapping the identity it expects of its root (spec: EP-13) changed a
//! device-local table, so both versions moved together and the window between
//! them is empty: no older file is opened at all, and the discard path below is
//! unreachable here. That is the intended consequence rather than an accident,
//! so the cases say it outright — an older file is refused whole and left
//! exactly as it was, which is what lets its mappings still be read out of it
//! and recorded again. Where the two edges of the window fall when it is not
//! empty is a unit case in the adapter's own `schema` module, against a
//! synthetic pair of versions.

use std::path::{Path, PathBuf};

use coffret_logging::testing::CapturedLogs;
use coffret_model::{ContainerKind, ContainerSummary, ContentHash, Mtime, ObjectRef};
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::device_state::{
    BatchId, DeviceTime, LocalObservation, Mapping, PendingUpload, RootIdentity, RootMarkerId,
    SpoolState,
};
use coffret_usecase::{Index, IndexError};
use tracing::Level;

mod support;

use support::{
    checkpoint, ciphertext_len, container_id, entry_path, generation, rows_in, snapshot, stamp_of,
    Scratch,
};

/// The layout this build writes, and the one its device-local group last
/// changed at.
///
/// Written out rather than read from the adapter, which keeps them to itself.
/// A case that moved with the constant would stop being a case about these two
/// numbers, and it is the numbers — here, two that are equal — that decide
/// everything below.
const SCHEMA_VERSION: i64 = 6;
const DEVICE_SCHEMA_VERSION: i64 = 6;

/// The layout before this one, which every case about an older file is written
/// against.
///
/// Under the previous pair its device-local group was one this build read, and
/// a file stamped with it had its catalog discarded and the rest kept. It is
/// the same number here and the answer is now a refusal, which is the whole of
/// what an empty window changes.
const PREVIOUS_SCHEMA_VERSION: i64 = SCHEMA_VERSION - 1;

/// Where one part of the Library lives on this device (spec: EP-9).
fn mapping() -> Mapping {
    // Stamped as a scan that has seen the root leaves it (spec: EP-12), and
    // expecting the identity a registration wrote into that root
    // (spec: EP-13): the two columns of this row that a file kept for its
    // device state must still hold afterwards.
    Mapping::new(
        Some(entry_path("albums")),
        PathBuf::from("/somewhere/albums"),
    )
    .stamped(RootIdentity::new("volume-7"))
    .expecting(RootMarkerId::from_bytes([
        0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77,
    ]))
}

/// One file this device has materialized (spec: EP-10).
fn observation() -> LocalObservation {
    LocalObservation {
        path: entry_path("albums/1.jpg"),
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
/// The stamp alone decides, before any table is looked at, so the shape the rows
/// were written in never comes into it: the stamp is the whole of what makes the
/// file an older one.
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
        content.containers(),
        [ContainerSummary {
            id: container_id(1),
            kind: ContainerKind::Pack,
            ciphertext_hash: ContentHash::from_bytes([1; ContentHash::BYTE_LEN]),
            ciphertext_len: ciphertext_len(164),
            object_ref: Some(ObjectRef::new("stored-1")),
        }]
    );
}

/// The layout before this one is refused whole, and nothing in it is discarded.
///
/// This is the case that used to watch a catalog be thrown away while the
/// device's own state was kept. The change that added the identity a mapping
/// expects of its root (spec: EP-13) moved the device-local floor up to the
/// current layout, so there is no longer any version at which half a file can be
/// kept: the window is empty, the discard path is unreachable, and a file stamped
/// with the previous layout is refused entire.
///
/// What that costs and what it does not is the point of the assertions. Nothing
/// is converted and nothing is thrown away — every row of both groups is still
/// in the file afterwards, and so is its stamp — which is what leaves the
/// mappings there to be read out by name and recorded again (see the
/// `refused_index` suite beside this one).
#[tokio::test]
async fn the_previous_layout_is_refused_whole_rather_than_half_discarded() {
    let scratch = Scratch::new();
    a_file_stamped(&scratch, PREVIOUS_SCHEMA_VERSION).await;

    let result = SqliteIndex::open(scratch.file());
    assert!(
        matches!(
            result.as_ref().err(),
            Some(IndexError::UnsupportedSchema { found, supported })
                if *found == PREVIOUS_SCHEMA_VERSION && *supported == SCHEMA_VERSION
        ),
        "expected the layout below an empty window to be refused, got {:?}",
        result.err()
    );

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
            "the refused file kept every row of {table}: the catalog was not discarded either"
        );
    }
    assert_eq!(
        stamp_of(&scratch.file()),
        PREVIOUS_SCHEMA_VERSION,
        "and the file is still stamped as the layout it was written to"
    );
}

/// What a refusal leaves the owner is a fresh file and the two gestures that
/// fill it, and neither of them is a repair path of its own.
///
/// There is no migration and there is nothing to convert: the catalog comes back
/// from Storage the way it comes back for a file that never existed (spec: RV-5),
/// and this device's own state is recorded again — `coffret map` for each mapping
/// the refused file still gives up. So the recovery is written here as what it
/// is, a fresh file that ends up holding exactly what any other fresh file would.
#[tokio::test]
async fn the_recovery_from_a_refused_file_is_a_fresh_one_the_catch_up_fills() {
    let older = Scratch::new();
    a_file_stamped(&older, PREVIOUS_SCHEMA_VERSION).await;
    assert!(
        SqliteIndex::open(older.file()).is_err(),
        "the file the owner is recovering from is one this build refuses"
    );

    let recovered_file = Scratch::new();
    let recovered = SqliteIndex::open(recovered_file.file()).expect("a fresh file must open");
    // The one gesture the owner makes by hand, over the mapping the refused file
    // still gives up by name.
    recovered
        .set_mapping(mapping())
        .await
        .expect("recording a mapping must succeed");

    let fresh_file = Scratch::new();
    let fresh = SqliteIndex::open(fresh_file.file()).expect("a fresh file must open");

    for index in [&recovered, &fresh] {
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
        recovered
            .snapshot()
            .await
            .expect("a caught-up catalog has a state to checkpoint"),
        fresh
            .snapshot()
            .await
            .expect("a caught-up catalog has a state to checkpoint"),
        "the same calls reach the same catalog, whatever was refused before them"
    );
    assert_eq!(
        recovered
            .mappings()
            .await
            .expect("reading the mappings must succeed"),
        vec![mapping()],
        "and the catch-up left the mapping the owner recorded where it was"
    );
    assert_eq!(
        stamp_of(&older.file()),
        PREVIOUS_SCHEMA_VERSION,
        "the refused file was never a step in any of it and is untouched"
    );
}

/// A layout from further back than the previous one is refused for the same
/// reason and in the same way.
///
/// The floor and the current layout coincide in this build, so "older than the
/// device state" now names every older layout rather than a range below a
/// window. The case stays at a version well below both, because what it is here
/// to say is that the answer does not change with how old the file is: one
/// refusal, and a file left exactly as it was found.
#[tokio::test]
async fn a_layout_older_than_the_device_state_is_refused() {
    let scratch = Scratch::new();
    let found = DEVICE_SCHEMA_VERSION - 2;
    a_file_stamped(&scratch, found).await;

    let result = SqliteIndex::open(scratch.file());
    assert!(
        matches!(
            result.as_ref().err(),
            Some(IndexError::UnsupportedSchema { found: refused, supported })
                if *refused == found && *supported == SCHEMA_VERSION
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
        found,
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

/// Two connections over one old file both refuse it, and neither changes it.
///
/// A server answering a browser while the same person runs a sync in a terminal
/// is two processes over one file, and both may reach an old one at the same
/// moment. This used to be about the second one finding the layout the first had
/// rebuilt; with the window empty there is no rebuild for it to find, and what
/// has to hold instead is that neither open is a half-step — the file the second
/// process reaches is the file the first one was refused by, and the owner's
/// recovery does not race two conversions.
#[tokio::test]
async fn a_second_connection_over_an_older_file_refuses_it_too() {
    let scratch = Scratch::new();
    a_file_stamped(&scratch, PREVIOUS_SCHEMA_VERSION).await;

    for attempt in ["the first", "the second"] {
        let result = SqliteIndex::open(scratch.file());
        assert!(
            matches!(
                result.as_ref().err(),
                Some(IndexError::UnsupportedSchema { found, .. })
                    if *found == PREVIOUS_SCHEMA_VERSION
            ),
            "expected {attempt} open to be refused, got {:?}",
            result.err()
        );
    }

    assert_eq!(
        stamp_of(&scratch.file()),
        PREVIOUS_SCHEMA_VERSION,
        "neither open stamped a layout of its own into the file"
    );
    assert_eq!(
        rows_in(&scratch.file(), "mappings"),
        1,
        "one mapping was recorded and one is there: neither open touched the group"
    );
}

/// Refusing an older file records nothing, because nothing was done to it.
///
/// The warning below belonged to the discard: it is what told an owner that a
/// catalog had been thrown away behind their back, an event nobody asked for and
/// nobody would otherwise see. A refusal is the opposite — it is handed straight
/// back to the caller, which reports it and the recovery for it in the caller's
/// own words — so there is nothing left here to warn about, and a warning would
/// say that something happened to a file that was left alone.
///
/// What the refusal itself may carry is unchanged: the Index lives under the
/// state directory and its path is the owner's own, so the two versions are the
/// whole of what it names, whether it reaches a log or a terminal.
#[tokio::test]
async fn refusing_an_older_layout_records_nothing_and_names_no_path() {
    let scratch = Scratch::new();
    a_file_stamped(&scratch, PREVIOUS_SCHEMA_VERSION).await;

    let logs = CapturedLogs::capture();
    let refusal = SqliteIndex::open(scratch.file())
        .err()
        .expect("the previous layout must be refused");

    assert!(
        logs.at(Level::WARN).is_empty(),
        "a file nothing was done to leaves nothing to report: {}",
        logs.text()
    );
    logs.assert_free_of(&[
        scratch
            .path()
            .to_str()
            .expect("a temporary directory's path is UTF-8 here"),
        "index.sqlite",
    ]);
    let said = refusal.to_string();
    assert!(
        !said.contains("index.sqlite"),
        "the refusal names the layout and not the file: {said}"
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
            .adopted_from(),
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
        content.adopted_from(),
        Some(&coffret_model::ControlObjectName::index_snapshot(
            generation(4)
        )),
        "the Snapshot this catalog started from is still what it started from"
    );
    assert_eq!(content.checkpoint().head_generation(), generation(5));
    assert_eq!(
        content.checkpoint().next_commit_slot(),
        Some("minted-5"),
        "the slot the head carries survives the file (spec: CP-2)"
    );
}

/// A path spelled with a combining acute rather than the composed character —
/// what no writer holding to EP-1 ever puts in a column.
const DECOMPOSED: &str = "cafe\u{301}.jpg";

/// A path with a `..` component in it — what no writer holding to EP-2 ever
/// puts in a column either, and what a path that climbed out of a mapped folder
/// would look like if one could reach the catalog.
const RELATIVE: &str = "../x";

/// Rewrites one text column of one table, behind the adapter's back.
///
/// The invariants make a path that is not NFC and a path outside EP-2's shape
/// both unbuildable through the port, which is the point of them, so a file
/// holding one is written at the SQL the adapter itself would have used.
fn overwrite(file: &Path, statement: &str, value: &str) {
    let connection = rusqlite::Connection::open(file).expect("the Index file must open");
    let changed = connection
        .execute(statement, [value])
        .expect("the statement must run");
    assert_eq!(changed, 1, "the case rewrote exactly one row");
}

/// The same, for a column that holds an integer.
fn overwrite_integer(file: &Path, statement: &str, value: i64) {
    let connection = rusqlite::Connection::open(file).expect("the Index file must open");
    let changed = connection
        .execute(statement, [value])
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
        DECOMPOSED,
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

/// A row whose `offset` and `size` end past what a plaintext stream can address
/// is a catalog this build cannot read (spec: FM-9, FM-19).
///
/// The pair places no Entry: there is no range of the stream that starts where
/// it says and runs as far as it says. No writer holding to FM-9 ever put such
/// a row in a column — the layout that lays a Container out refuses the table
/// before the object is written — so a file holding one is a file this build
/// refuses, the way it refuses one holding a malformed path. The catalog is a
/// cache of what Storage holds (spec: RV-5), so that costs a rebuild and
/// nothing else, where answering with the row would hand a fetch a range no
/// Container could ever be read from.
#[tokio::test]
async fn a_row_whose_extent_passes_the_end_of_the_address_space_makes_the_catalog_unreadable() {
    let scratch = Scratch::new();

    {
        let index = SqliteIndex::open(scratch.file()).expect("a fresh file must open");
        index
            .restore(snapshot(4))
            .await
            .expect("restoring a Snapshot must succeed");
    }
    // The row's own size is 100, so an offset fifty short of the last position
    // the format admits (FM-19) ends fifty bytes past it. The column holds that
    // offset as an ordinary positive integer, so what the read refuses is the
    // extent and not a sign no writer could have put there.
    overwrite_integer(
        &scratch.file(),
        "UPDATE entries SET \"offset\" = ?1 WHERE path = (SELECT min(path) FROM entries)",
        i64::MAX - 50,
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
        "expected an extent past the address space to make the catalog unreadable, got {result:?}"
    );
}

/// A stored Entry Path outside the shape every Entry Path is in is the same
/// verdict as a decomposed one: a catalog this build cannot read (spec: EP-2).
///
/// It matters here more than the normal form does, because the catalog is the
/// one place such a path could have been written before the shape was the
/// type's — and a catalog answering `entries_under` with `../x` would hand a
/// fetch a path to climb out of a mapped folder with. The catalog is a cache of
/// what Storage holds (spec: RV-5), so refusing it costs a rebuild and nothing
/// else.
#[tokio::test]
async fn a_row_whose_path_has_a_shape_ep_2_excludes_makes_the_catalog_unreadable() {
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
        RELATIVE,
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
        "expected a `..` component to make the catalog unreadable, got {result:?}"
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
            .set_mapping(Mapping::new(
                Some(entry_path("albums")),
                PathBuf::from("/tmp/albums"),
            ))
            .await
            .expect("recording a mapping must succeed");
    }
    overwrite(
        &scratch.file(),
        "UPDATE mappings SET prefix = ?1",
        DECOMPOSED,
    );

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

/// A `checkpoint` row whose Journal generation is past its head is a catalog
/// this build cannot read (spec: CK-1).
///
/// The two generations coincide after an ordinary commit and diverge only
/// downwards, at an epoch activation whose Snapshot takes a head position
/// without being a Journal record (spec: CP-6). A row saying otherwise names
/// records applied to reach a state the head does not cover, which no commit
/// produces — so no writer holding to CK-1 ever put it there. Answering with it
/// would send the next catch-up replaying from a starting point its own
/// checkpoint does not reach; the catalog is a cache of what Storage holds
/// (spec: RV-5), so refusing it costs a rebuild and nothing else.
#[tokio::test]
async fn a_checkpoint_row_whose_journal_is_ahead_of_its_head_makes_the_catalog_unreadable() {
    let scratch = Scratch::new();

    {
        let index = SqliteIndex::open(scratch.file()).expect("a fresh file must open");
        index
            .restore(snapshot(4))
            .await
            .expect("restoring a Snapshot must succeed");
    }
    overwrite_integer(
        &scratch.file(),
        "UPDATE checkpoint SET journal_generation = ?1",
        5,
    );

    let index = SqliteIndex::open(scratch.file()).expect("an existing file must reopen");
    let result = index.checkpoint().await;
    assert!(
        matches!(
            result.as_ref().err(),
            Some(IndexError::UnreadableCatalog {
                operation: "reading the checkpoint",
                ..
            })
        ),
        "expected a Journal generation past the head to make the catalog unreadable, got {result:?}"
    );
}
