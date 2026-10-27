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
//! In *this* build that line falls at layout 20: layout 21 changed only the
//! catalog — its checkpoint row holds the Library's name — so a layout-20 file
//! keeps everything it holds, catalog included, and its checkpoint row gains
//! the name, unset. Layout 20 had changed the device group — each reservation
//! says whether its drop finished receiving — so every older layout is brought
//! forward rather than read as it is. A layout-19 file keeps everything it
//! holds too, its checkpoint row gains the name, and its reservations gain the
//! mark unset, each read as a drop cut off part way. A layout-18 file takes the
//! same step and loses its catalog, whose Entries carry no deposit time, which
//! the next catch-up rebuilds from Storage. Every older layout loses its
//! catalog too, after its device group is brought forward. Layout 18 had
//! changed the device group: it keeps the reservations a drop's choice of a
//! Pack is recorded in, so a layout-17 file keeps every row it holds and gains
//! the reservations, empty.
//! Layout 17 had changed it before: a materialized file's row names the Entry
//! it is a copy of by its Container and entry number. A layout-16 file has
//! each row tied to the Entry its catalog holds where the row stands, a row it
//! holds none for dropped and reported; every layout from 7 to 15 has its
//! group upgraded by the adapter's own provenance step, tied the same way
//! where its catalog numbers its Entries. The layouts below 7
//! used here are refused whole and left exactly as they were, which is what
//! lets their mappings still be read out of them and recorded again. Where the
//! two edges of the window fall is a unit case in the adapter's own `schema`
//! module, against a synthetic pair of versions.

use std::path::{Path, PathBuf};

use coffret_logging::testing::CapturedLogs;
use coffret_model::{ContainerKind, ContainerSummary, ContentHash, EntryNumber, Mtime, ObjectRef};
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::device_state::{
    BatchId, DeviceTime, LocalObservation, Mapping, PendingRow, RootIdentity, RootMarkerId,
    SpoolState,
};
use coffret_usecase::{EntryRef, Index, IndexError};
use tracing::Level;

mod support;

use support::{
    checkpoint, ciphertext_len, container_id, entry_path, generation, rows_in, snapshot, stamp_of,
    Scratch,
};

/// The layout this build writes, the one its device-local group last changed
/// at, and the oldest whose device-local group it upgrades in place.
///
/// Written out rather than read from the adapter, which keeps them to itself.
/// A case that moved with the constants would stop being a case about these
/// numbers, and it is the numbers that decide everything below: a file at
/// [`DEVICE_SCHEMA_VERSION`] or above is read as it is, one from
/// [`OLDEST_UPGRADED_SCHEMA_VERSION`] up to the layout before that takes an
/// upgrade, and one older than that is refused.
const SCHEMA_VERSION: i64 = 21;
const DEVICE_SCHEMA_VERSION: i64 = 20;
const OLDEST_UPGRADED_SCHEMA_VERSION: i64 = 7;

/// The newest layout this build refuses for being too old, which every case
/// about a refused older file is written against.
///
/// It sits right below the oldest layout the upgrade takes, so a refusal here
/// is the edge of what this build reads rather than a version picked from far
/// below it.
const REFUSED_SCHEMA_VERSION: i64 = OLDEST_UPGRADED_SCHEMA_VERSION - 1;

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

/// One file this device has materialized (spec: EP-10): a copy of the first
/// Entry of the Pack the seeded Snapshot holds, at that Entry's path.
fn observation() -> LocalObservation {
    LocalObservation {
        path: entry_path("albums/1.jpg"),
        size: 100,
        mtime: Mtime::from_unix_seconds(1_700_000_000),
        at: DeviceTime::from_unix_seconds(1_700_000_400),
        entry: EntryRef::new(container_id(1), EntryNumber::FIRST),
        hash: Some(ContentHash::from_bytes([0x3c; ContentHash::BYTE_LEN])),
    }
}

/// One pending row an interrupted run left behind, which nothing outside this
/// file records (spec: OC-2, OC-7).
fn pending() -> PendingRow {
    PendingRow {
        commit_attempted: false,
        materializes: true,
        container_id: container_id(9),
        spool_path: PathBuf::from("/somewhere/spool/9.pack"),
        state: SpoolState::Spooled(Some(ObjectRef::new("stored-9"))),
        batch: BatchId::new("batch-9"),
        created_at: DeviceTime::from_unix_seconds(1_700_000_500),
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
            .record_pending_row(pending())
            .await
            .expect("recording a spool must succeed");
    }
    restamp(&scratch.file(), version);
}

/// Stamps a layout version into a file, behind the adapter's back.
///
/// An older layout's file loses the Library's name layout 21 added to the
/// checkpoint row, the reservations' received mark layout 20 added, the
/// catalog's deposit times layout 19 added, the device-local tables and
/// columns layouts 16 to 18 added, and one before 15 the entry numbers of
/// its catalog, so the upgrade meets the shape a build before it left behind.
fn restamp(file: &Path, version: i64) {
    let connection = rusqlite::Connection::open(file).expect("the Index file must open");
    if version < 21 {
        connection
            .execute_batch("ALTER TABLE checkpoint DROP COLUMN library_name;")
            .expect("dropping a column must succeed");
    }
    if version < 20 {
        connection
            .execute_batch("ALTER TABLE reservations DROP COLUMN received;")
            .expect("dropping a column must succeed");
    }
    if version < 19 {
        connection
            .execute_batch("ALTER TABLE entries DROP COLUMN deposited_at;")
            .expect("dropping a column must succeed");
    }
    if version < 18 {
        connection
            .execute_batch("DROP TABLE reserved_paths; DROP TABLE reservations;")
            .expect("dropping a table must succeed");
    }
    if version < 17 {
        connection
            .execute_batch(
                "DROP INDEX local_entries_by_entry;                  ALTER TABLE local_entries DROP COLUMN entry_container;                  ALTER TABLE local_entries DROP COLUMN entry_number;",
            )
            .expect("dropping a column must succeed");
    }
    if version < 16 {
        connection
            .execute_batch("ALTER TABLE local_entries DROP COLUMN relocate_to;")
            .expect("dropping a column must succeed");
    }
    if version < 15 {
        connection
            .execute_batch(
                "DROP INDEX entries_by_container; ALTER TABLE entries DROP COLUMN entry_number;",
            )
            .expect("dropping a column must succeed");
    }
    connection
        .pragma_update(None, "user_version", version)
        .expect("stamping a version must succeed");
}

/// Makes a reservation of `albums` naming one file, as a drop does, in a file
/// that is then stamped as an older layout.
async fn a_reservation_in(scratch: &Scratch) {
    let index = SqliteIndex::open(scratch.file()).expect("the file must open");
    let id = index
        .reserve(Some(&entry_path("albums")))
        .await
        .expect("recording a reservation must succeed");
    index
        .extend_reservation(id, &entry_path("albums/2.jpg"))
        .await
        .expect("extending it must succeed");
}

/// The reservation [`a_reservation_in`] made, read back from the upgraded file
/// as a drop in progress: no build before layout 20 said a drop had finished
/// receiving, so each one an older file holds is read as cut off part way
/// (spec: PK-17, LA-12).
async fn reads_its_reservation_as_cut_off(index: &SqliteIndex) {
    let reservations = index
        .reservations()
        .await
        .expect("reading the reservations must succeed");
    assert_eq!(reservations.len(), 1, "the reservation is kept");
    let kept = &reservations[0];
    assert_eq!(kept.folder, Some(entry_path("albums")));
    assert_eq!(
        kept.paths
            .iter()
            .map(|path| path.as_str())
            .collect::<Vec<_>>(),
        ["albums/2.jpg"],
        "with every path it named",
    );
    assert!(
        kept.in_progress(),
        "and read as a drop that never finished receiving: {kept:?}",
    );
    index
        .mark_received(kept.id)
        .await
        .expect("the mark is one the port writes like a fresh file's");
    assert!(index
        .reservations()
        .await
        .expect("reading the reservations must succeed")
        .iter()
        .all(|reservation| reservation.received));
}

/// The layout before this one keeps every row it holds, its catalog included —
/// layout 21 changed only the catalog's checkpoint row — and that row gains
/// the Library's name, unset: the next open reads the name from Storage
/// (spec: FM-15). Nothing is discarded, so nothing is reported.
#[tokio::test]
async fn the_layout_before_this_one_keeps_its_catalog_and_names_nothing_yet() {
    assert_eq!(
        DEVICE_SCHEMA_VERSION,
        SCHEMA_VERSION - 1,
        "the device-local floor is the layout before this one"
    );
    let scratch = Scratch::new();
    a_file_stamped(&scratch, SCHEMA_VERSION).await;
    a_reservation_in(&scratch).await;
    restamp(&scratch.file(), SCHEMA_VERSION - 1);

    let logs = CapturedLogs::capture();
    let index = SqliteIndex::open(scratch.file()).expect("layout 20 must open");
    keeps_everything(&index).await;
    assert_eq!(
        index
            .library_name()
            .await
            .expect("reading the name must succeed"),
        None,
        "and names nothing yet"
    );
    let reservations = index
        .reservations()
        .await
        .expect("reading the reservations must succeed");
    assert_eq!(reservations.len(), 1, "the reservation is kept");
    assert_eq!(logs.at(Level::WARN).len(), 0, "{}", logs.text());
    assert_eq!(stamp_of(&scratch.file()), SCHEMA_VERSION);
}

/// The layout two before this one keeps every row it holds, its catalog
/// included, its checkpoint row gains the Library's name as the layout after it
/// does, and its reservations gain the mark of a finished drop, unset
/// (spec: PK-17, LA-12, FM-15). Nothing is discarded, so nothing is reported.
#[tokio::test]
async fn the_layout_two_before_this_one_keeps_its_catalog_and_reads_its_reservations_as_cut_off() {
    let scratch = Scratch::new();
    a_file_stamped(&scratch, SCHEMA_VERSION).await;
    a_reservation_in(&scratch).await;
    restamp(&scratch.file(), SCHEMA_VERSION - 2);

    let logs = CapturedLogs::capture();
    let index = SqliteIndex::open(scratch.file()).expect("layout 19 must open");
    keeps_everything(&index).await;
    reads_its_reservation_as_cut_off(&index).await;
    assert_eq!(logs.at(Level::WARN).len(), 0, "{}", logs.text());
    assert_eq!(stamp_of(&scratch.file()), SCHEMA_VERSION);
}

/// What [`a_file_stamped`] put in, read back from a file that kept it all.
async fn keeps_everything(index: &SqliteIndex) {
    assert!(
        index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed")
            .is_some(),
        "the catalog is kept"
    );
    assert_eq!(
        index
            .mappings()
            .await
            .expect("reading the mappings must succeed"),
        vec![mapping()],
        "and so is the device's own mapping"
    );
    assert!(index
        .local_entry_at(&observation().path)
        .await
        .expect("reading a local file's row must succeed")
        .is_some());
}

/// The layout three before this one takes the same step for its reservations
/// and then loses its catalog, whose Entries carry no deposit time, keeping
/// every row of its device-local group (spec: RV-5, FM-15, PK-17).
#[tokio::test]
async fn the_layout_three_before_this_one_reads_its_reservations_as_cut_off_and_loses_its_catalog()
{
    loses_its_catalog(SCHEMA_VERSION - 3, Kept::Records).await;

    let scratch = Scratch::new();
    a_file_stamped(&scratch, SCHEMA_VERSION).await;
    a_reservation_in(&scratch).await;
    restamp(&scratch.file(), SCHEMA_VERSION - 3);
    let index = SqliteIndex::open(scratch.file()).expect("layout 18 must open");
    reads_its_reservation_as_cut_off(&index).await;
}

/// The layout four before this one keeps every row of its device-local group,
/// gains the reservations, holding none — no build before it recorded a drop's
/// choice of a Pack (spec: PK-17) — and loses its catalog as the layout after
/// it does (spec: RV-5, FM-15).
///
/// The reservations it gains are ones the port reads and writes like a fresh
/// file's.
#[tokio::test]
async fn the_layout_four_before_this_one_gains_no_reservation_and_loses_its_catalog() {
    loses_its_catalog(SCHEMA_VERSION - 4, Kept::Records).await;

    let scratch = Scratch::new();
    a_file_stamped(&scratch, SCHEMA_VERSION - 4).await;
    let index = SqliteIndex::open(scratch.file()).expect("layout 17 must open");
    assert_eq!(
        index
            .reservations()
            .await
            .expect("reading the reservations must succeed"),
        Vec::new(),
    );
    let reserved = index
        .reserve(Some(&entry_path("albums")))
        .await
        .expect("recording a reservation in the upgraded file must succeed");
    index
        .extend_reservation(reserved, &entry_path("albums/2.jpg"))
        .await
        .expect("extending it must succeed");
    assert_eq!(
        index
            .reservations()
            .await
            .expect("reading the reservations must succeed")
            .len(),
        1,
    );
}

/// The layout five before this one has each materialization record tied to
/// the Entry its catalog holds where it stands (spec: EP-10), and then loses
/// that catalog as every older layout does (spec: RV-5, FM-15).
///
/// Nothing is dropped, so the discard is the one thing reported.
#[tokio::test]
async fn the_layout_five_before_this_one_names_each_records_entry_and_loses_its_catalog() {
    loses_its_catalog(SCHEMA_VERSION - 5, Kept::Records).await;
}

/// A layout-16 record the catalog holds no Entry for — the file of an Entry
/// that has left the Library — is dropped rather than guessed at, and the drop
/// is reported with how many and nothing that names a file (spec: EP-10,
/// EL-1). One waiting to follow a rename is tied to the Entry where it waits.
#[tokio::test]
async fn a_record_the_catalog_names_no_entry_for_is_dropped_and_reported() {
    let scratch = Scratch::new();
    {
        let index = SqliteIndex::open(scratch.file()).expect("a fresh file must open");
        index
            .restore(snapshot(4))
            .await
            .expect("restoring a Snapshot must succeed");
        for observed in [
            observation(),
            // Its Entry, in a Container the catalog no longer holds, has left.
            LocalObservation {
                path: entry_path("albums/departed.jpg"),
                entry: EntryRef::new(container_id(9), EntryNumber::FIRST),
                ..observation()
            },
            LocalObservation {
                path: entry_path("inbox/waiting.jpg"),
                entry: EntryRef::new(
                    container_id(1),
                    EntryNumber::new(1).expect("one is an entry number"),
                ),
                ..observation()
            },
        ] {
            index
                .mark_present(observed)
                .await
                .expect("recording a materialized file must succeed");
        }
    }
    restamp(&scratch.file(), SCHEMA_VERSION - 5);

    let logs = CapturedLogs::capture();
    let index = SqliteIndex::open(scratch.file()).expect("layout 16 must open");
    assert!(
        index
            .local_entry_at(&entry_path("albums/departed.jpg"))
            .await
            .expect("reading a local file's row must succeed")
            .is_none(),
        "a record no Entry stands for is dropped"
    );
    let waiting = index
        .local_entry_at(&entry_path("inbox/waiting.jpg"))
        .await
        .expect("reading a local file's row must succeed")
        .expect("a record waiting to follow its Entry is kept");
    assert_eq!(
        waiting.observation.entry,
        EntryRef::new(
            container_id(1),
            EntryNumber::new(1).expect("one is an entry number")
        )
    );
    assert_eq!(waiting.relocate_to, Some(entry_path("albums/1.thumb.jpg")));
    assert!(index
        .local_entry_at(&observation().path)
        .await
        .expect("reading a local file's row must succeed")
        .is_some());
    // The drop and the discard of the catalog are each reported.
    let warned = logs.at(Level::WARN);
    assert_eq!(warned.len(), 2, "{}", logs.text());
    assert!(logs.text().contains("\"dropped\":1"), "{}", logs.text());
    assert!(!logs.text().contains("departed"), "{}", logs.text());
}

/// The layout six below this one has its device-local group upgraded and its
/// catalog discarded rather than migrated (spec: RV-5, EP-9, EP-10, OC-2).
///
/// Its device group lacks the columns that say which files wait to follow a
/// rename and which Entry each is a copy of; none waits, since no build before
/// layout 16 applied a rename, and each is named by the Entry its catalog
/// numbers at its path before that catalog goes. The catalog would still read,
/// and is discarded with every older layout's: the file is stamped as the
/// current layout with an empty catalog, which the next catch-up rebuilds from
/// Storage, and a warning says so.
#[tokio::test]
async fn the_layout_six_before_this_one_loses_its_catalog_and_keeps_its_device_state() {
    loses_its_catalog(SCHEMA_VERSION - 6, Kept::Records).await;
}

/// The layout seven below this one takes the same upgrade (spec: RV-5, EP-9,
/// EP-10, OC-2), and its catalog holds no entry numbers, so it cannot say
/// which Entry any record is a copy of: every record is dropped, and that is
/// reported beside the discard (spec: FM-16).
#[tokio::test]
async fn the_layout_seven_before_this_one_loses_its_catalog_and_its_records() {
    loses_its_catalog(SCHEMA_VERSION - 7, Kept::NoRecords).await;
}

/// Whether an upgrade keeps the materialization records.
enum Kept {
    Records,
    NoRecords,
}

/// Opens a file stamped `version`, and checks that its catalog was discarded,
/// its device group kept — its records only where `kept` says — and the file
/// stamped as the current layout.
async fn loses_its_catalog(version: i64, kept: Kept) {
    let scratch = Scratch::new();
    // The stamp alone decides, as it does for every case here: what the
    // catalog's columns were called is not looked at before it is dropped.
    a_file_stamped(&scratch, version).await;

    let logs = CapturedLogs::capture();
    let index = SqliteIndex::open(scratch.file()).expect("an older layout from 7 up must open");
    assert!(
        index
            .checkpoint()
            .await
            .expect("reading the checkpoint must succeed")
            .is_none(),
        "the catalog was discarded, not migrated"
    );
    assert_eq!(
        index
            .mappings()
            .await
            .expect("reading the mappings must succeed"),
        vec![mapping()],
        "the device's own mapping is kept"
    );
    assert_eq!(
        index
            .pending_rows()
            .await
            .expect("reading the pending rows must succeed")
            .len(),
        1,
        "and so is the spool it has not committed"
    );
    let materialized = index
        .local_entry_at(&observation().path)
        .await
        .expect("reading a local file's row must succeed");
    match kept {
        Kept::Records => {
            let materialized = materialized.expect("and so is the file it has on disk");
            assert_eq!(materialized.observation, observation());
            assert_eq!(
                materialized.relocate_to, None,
                "which waits to follow no rename"
            );
            assert_eq!(logs.at(Level::WARN).len(), 1, "{}", logs.text());
        }
        Kept::NoRecords => {
            assert!(
                materialized.is_none(),
                "a record no numbered catalog names an Entry for is dropped"
            );
            assert_eq!(
                logs.at(Level::WARN).len(),
                2,
                "the discard and the drop are each reported: {}",
                logs.text()
            );
        }
    }
    assert_eq!(stamp_of(&scratch.file()), SCHEMA_VERSION);
    assert_eq!(rows_in(&scratch.file(), "entries"), 0);

    index
        .restore(snapshot(4))
        .await
        .expect("the rebuilt catalog takes a Snapshot like a fresh one");
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

/// The newest layout older than the upgrades is refused whole, and nothing in
/// it is discarded.
///
/// No layout below [`OLDEST_UPGRADED_SCHEMA_VERSION`] has a device-local group
/// this build reads or upgrades, and Storage cannot reconstruct mappings,
/// materializations or provenance, so there is no version below it at which
/// half a file can be kept: a file stamped with such a layout is refused
/// entire.
///
/// What that costs and what it does not is the point of the assertions. Nothing
/// is converted and nothing is thrown away — every row of both groups is still
/// in the file afterwards, and so is its stamp — which is what leaves the
/// mappings there to be read out by name and recorded again (see the
/// `refused_index` suite beside this one).
#[tokio::test]
async fn a_layout_older_than_the_upgrades_is_refused_whole_rather_than_half_discarded() {
    let scratch = Scratch::new();
    a_file_stamped(&scratch, REFUSED_SCHEMA_VERSION).await;

    let result = SqliteIndex::open(scratch.file());
    assert!(
        matches!(
            result.as_ref().err(),
            Some(IndexError::UnsupportedSchema { found, supported })
                if *found == REFUSED_SCHEMA_VERSION && *supported == SCHEMA_VERSION
        ),
        "expected the layout below the oldest upgraded one to be refused, got {:?}",
        result.err()
    );

    for (table, rows) in [
        ("checkpoint", 1),
        ("containers", 1),
        ("entries", 2),
        ("mappings", 1),
        ("local_entries", 1),
        ("pending_rows", 1),
    ] {
        assert_eq!(
            rows_in(&scratch.file(), table),
            rows,
            "the refused file kept every row of {table}: the catalog was not discarded either"
        );
    }
    assert_eq!(
        stamp_of(&scratch.file()),
        REFUSED_SCHEMA_VERSION,
        "and the file is still stamped as the layout it was written to"
    );
}

/// A new Index can reconstruct the Catalog but not another file's device
/// records. The refused original must remain intact (spec: RV-5, OC-2).
#[tokio::test]
async fn a_fresh_index_rebuilds_only_the_catalog() {
    let older = Scratch::new();
    a_file_stamped(&older, REFUSED_SCHEMA_VERSION).await;
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
    assert!(recovered.pending_rows().await.unwrap().is_empty());
    let original = rusqlite::Connection::open(older.file()).unwrap();
    let retained: i64 = original
        .query_row("SELECT count(*) FROM pending_rows", [], |row| row.get(0))
        .unwrap();
    assert_eq!(
        retained, 1,
        "the original still holds provenance Storage cannot reconstruct"
    );
    assert_eq!(
        stamp_of(&older.file()),
        REFUSED_SCHEMA_VERSION,
        "the refused file was never a step in any of it and is untouched"
    );
}

/// A layout from further back than the newest refused one is refused for the
/// same reason and in the same way.
///
/// The case stays at a version well below the oldest upgraded layout, because
/// what it is here to say is that the answer does not change with how old the
/// file is: one refusal, and a file left exactly as it was found.
#[tokio::test]
async fn a_layout_further_back_is_refused_the_same_way() {
    let scratch = Scratch::new();
    let found = OLDEST_UPGRADED_SCHEMA_VERSION - 2;
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
        ("pending_rows", 1),
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
/// moment. For a layout this build refuses there is no rebuild for the second
/// to find, and what has to hold is that neither open is a half-step — the file
/// the second process reaches is the file the first one was refused by, and the
/// owner's recovery does not race two conversions.
#[tokio::test]
async fn a_second_connection_over_an_older_file_refuses_it_too() {
    let scratch = Scratch::new();
    a_file_stamped(&scratch, REFUSED_SCHEMA_VERSION).await;

    for attempt in ["the first", "the second"] {
        let result = SqliteIndex::open(scratch.file());
        assert!(
            matches!(
                result.as_ref().err(),
                Some(IndexError::UnsupportedSchema { found, .. })
                    if *found == REFUSED_SCHEMA_VERSION
            ),
            "expected {attempt} open to be refused, got {:?}",
            result.err()
        );
    }

    assert_eq!(
        stamp_of(&scratch.file()),
        REFUSED_SCHEMA_VERSION,
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
    a_file_stamped(&scratch, REFUSED_SCHEMA_VERSION).await;

    let logs = CapturedLogs::capture();
    let refusal = SqliteIndex::open(scratch.file())
        .err()
        .expect("a layout older than the upgrades must be refused");

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
    // the format admits (spec: FM-19) ends fifty bytes past it. The column
    // holds that offset as an ordinary positive integer, so what the read
    // refuses is the extent and not a sign no writer could have put there.
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
