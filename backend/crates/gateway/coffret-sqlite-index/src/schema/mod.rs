mod provenance;

// The upgrades that add the reservations a drop's choice of a Pack is kept in,
// which a layout-17 file takes on its way to this one, and the mark saying
// whether a reservation's drop finished receiving, which a layout-18 or -19
// file takes (spec: PK-17).
mod reservations;

// The upgrade that ties each materialization record to the Entry it records,
// which every older layout takes on its way to this one (spec: EP-10).
mod entry_identities;

// The upgrade that gives the checkpoint row the Library's name, which a
// layout-20 file takes in place and a layout-19 one beside its own upgrade
// (spec: FM-15, FM-16).
mod library_name;

use std::ops::Range;

use coffret_usecase::{IndexError, IndexResult};
use rusqlite::{Connection, TransactionBehavior};

use crate::error::classify;

/// Current layout. Version 8 adds the durable commit-attempt marker, version 9
/// the marker saying whether a pending Container's Entries are files this
/// device put into it, version 10 the content hash a materialized file was
/// made to match, version 11 the parcels this device holds (spec: PK-21),
/// version 12 the commit slots a batch attempted (spec: OC-2, OC-3), version 13
/// each Entry's captured path beside its current one (spec: FM-15, FM-16),
/// version 14 a catalog filled from Storage objects whose maps carry their
/// fields under field numbers, with that column under the name the
/// specification now gives the field, `captured_path` (spec: FM-9), version 15
/// each Entry's entry number in its Container (spec: FM-9, FM-16), and version
/// 16 the Entry Path a materialized file waits to follow once a rename has
/// changed its Entry's current path (spec: FM-15, EP-10), version 17 the
/// Entry each materialized file is a copy of, by its Container and entry number
/// (spec: FM-9, EP-10), version 18 the reservations a drop's choice of a
/// Pack is kept in until a freeze has decided its files (spec: PK-17),
/// version 19 each Entry's deposit time (spec: FM-15, FM-16), and version 20
/// the mark saying whether a reservation's drop finished receiving
/// (spec: PK-17, LA-12), and version 21 the Library's name on the checkpoint
/// row (spec: FM-15, FM-16).
pub(crate) const SCHEMA_VERSION: i64 = 21;

/// The device-local layout floor: the oldest layout whose device-local group is
/// the one this build writes.
///
/// Layout 20 last changed the device-local group, so the floor is layout 20,
/// and layout 21 changed the catalog alone: a layout-20 file keeps every row it
/// holds, catalog included, and its checkpoint row gains the Library's name,
/// unset — the next open reads the name from Storage before giving the Library
/// one (spec: FM-15). No older layout is read as it is: each takes an upgrade.
/// A layout-19 file keeps every row it holds, catalog included, its checkpoint
/// row gains the name the same way, and its reservations gain the mark unset —
/// a build before it never said a drop had finished receiving, so each
/// reservation it left is read as a drop cut off part way, which waits for its
/// folder to be dropped again rather than being packed as it stands
/// (spec: PK-17, LA-12). A layout-18 file takes the same step for its
/// reservations and loses its catalog, whose Entries carry no deposit time and
/// were filled from objects in a format this build no longer reads, so the
/// rebuild from Storage refills it (spec: RV-5, FM-15). A layout-17 file keeps
/// every row of its
/// device group, gains the tables of reservations, empty — no build before it
/// recorded a drop's choice of a Pack, so no file it wrote waits for one
/// (spec: PK-17) — and loses its catalog for the same reason. A layout-16 file
/// has each materialization record tied to the Entry the catalog holds where
/// the record stands, or where it waits to follow (spec: EP-10), and then
/// loses its catalog the same way; a record the catalog holds no Entry for is
/// dropped and the drop reported, never guessed at. A layout from 13 to 15 has
/// the columns it lacks added by the same upgrade the older ones take, and
/// loses its catalog with them: a layout-13 catalog was filled from objects in
/// a format this build no longer reads, a layout-14 one holds no
/// entry numbers, and nothing either holds says what they were, so the rebuild
/// from Storage replays them out of the Journal (spec: RV-5, FM-16). A
/// layout-15 catalog would still read, and is discarded all the same: the
/// catalog is a cache, and one path for every older file is one path to keep
/// right.
///
/// Versions 7 to 15 have one explicit, transactional upgrade of their
/// device-local group that preserves every row, treats old attempts as
/// unknown, takes every old pending row to name a Container built out of this
/// device's own files — the only kind a build before version 9 could spool —
/// leaves every old materialization without a recorded hash, which is what a
/// build before version 10 never wrote down, starts with no parcel held, which
/// is what a build before version 11 never kept, records no commit slot for
/// any batch, which is what a build before version 12 never kept — so an old
/// attempt stays one whose outcome is unknown (spec: OC-2) — and has no file
/// waiting to follow a rename, which no build before version 16 applied, and
/// no reservation, which no build before version 18 recorded. Each
/// of their materialization records is tied to its Entry the way a layout-16
/// one is, from a catalog that numbers its Entries — layout 15's — and dropped
/// with a report where the catalog cannot say: a layout before 15 numbers none
/// (spec: FM-16). Their catalog is then discarded in the same transaction, for
/// the reason above. Other
/// unreadable device layouts are refused rather than discarded, because
/// Storage cannot reconstruct mappings, materializations, or provenance.
pub(crate) const DEVICE_SCHEMA_VERSION: i64 = 20;

/// The table of current Entries, part of the Library-wide group below.
macro_rules! entries_ddl {
    () => {
        r#"
CREATE TABLE entries (
    -- UNIQUE by being the primary key: a defence, not the enforcement. What
    -- keeps one Entry Path to one current Entry is the commit's own check
    -- (spec: EP-5, EP-6); this catches a record that reached the catalog
    -- claiming otherwise.
    path                   TEXT PRIMARY KEY COLLATE BINARY,
    -- The Entry Path the owning Container's meta section records for this
    -- Entry: the name it was captured under when that Container was written,
    -- which no rename changes, where `path` is where it stands now
    -- (spec: FM-9, FM-15, FM-16).
    captured_path          TEXT NOT NULL COLLATE BINARY,
    container_id           BLOB NOT NULL REFERENCES containers(id),
    -- The Entry's 0-based position in its Container's entry table: with
    -- `container_id`, the name for it that a rename leaves unchanged
    -- (spec: FM-9).
    entry_number           INTEGER NOT NULL,
    "offset"               INTEGER NOT NULL,
    size                   INTEGER NOT NULL,
    mtime                  INTEGER NOT NULL,
    -- NULL where the platform that wrote the Container reported no birth time,
    -- which is the whole of what absent means (spec: FM-9, FM-15).
    btime                  INTEGER,
    -- When the content now at `path` was deposited into the Library, in Unix
    -- seconds: the catalog's own value, which no Container records, carried
    -- unchanged by a rename and by a rewrite that carries the content
    -- (spec: FM-15, FM-16).
    deposited_at           INTEGER NOT NULL,
    hash                   BLOB NOT NULL,
    mime                   TEXT,
    derived_from_container BLOB,
    derived_from_path      TEXT COLLATE BINARY
) STRICT;

-- Removing a Container removes its Entries, and a Pack holds many; the
-- entry number beside it finds one Entry of a Container by its position.
CREATE INDEX entries_by_container ON entries (container_id, entry_number);
"#
    };
}

/// The group an Index Snapshot carries: the whole Library, identical on every
/// enrolled device (spec: CK-7).
///
/// It stands on its own so that it can be laid out on its own, which is what a
/// discard does with it — the boundary between the two groups is a real one
/// rather than a comment somebody has to honour.
///
/// Every Entry Path is stored in its canonical form (spec: EP-1, EP-2) and
/// compared bytewise: `BINARY` is spelled out on each path column.
const LIBRARY_WIDE_DDL: &str = concat!(
    r#"
CREATE TABLE checkpoint (
    -- One row: an Index stands at exactly one committed Library state.
    only_row              INTEGER PRIMARY KEY CHECK (only_row = 0),
    master_key_epoch      INTEGER NOT NULL,
    head_generation       INTEGER NOT NULL,
    journal_generation    INTEGER NOT NULL,
    -- Storage's own opaque token, and NULL where the provider mints none
    -- (spec: CP-2).
    next_commit_slot      TEXT,
    keyring_generation    INTEGER NOT NULL,
    keyring_replica_count INTEGER NOT NULL,
    keyring_set_digest    TEXT NOT NULL,
    -- The checkpoint object this content was adopted from, NULL on a catalog
    -- that has only ever replayed records (spec: CK-9).
    adopted_snapshot      TEXT,
    -- The Library's name as the latest record or adopted Snapshot carrying one
    -- gave it; NULL where none did (spec: FM-16). No comma in this comment:
    -- dropping the last column of a table cuts back to the last comma before it.
    library_name          TEXT
) STRICT;

CREATE TABLE containers (
    id              BLOB PRIMARY KEY,
    kind            TEXT NOT NULL,
    ciphertext_hash BLOB NOT NULL,
    ciphertext_len  INTEGER NOT NULL,
    object_ref      TEXT
) STRICT;
"#,
    entries_ddl!()
);

/// The group that is this device's alone and is never uploaded (spec: EP-9,
/// EP-10, OC-2).
///
/// Never in a Snapshot (spec: CK-7), and so never rebuilt from one: this is the
/// only place any of it is written down. Nothing but creating a file lays it
/// out, and a discard does not so much as read it.
///
/// Entry Paths here are canonical and compared bytewise for the reason they are
/// above.
const DEVICE_LOCAL_DDL: &str = r#"
CREATE TABLE mappings (
    -- NULL is the Library root. A device may have at most one root mapping and
    -- at most one per top-level component (spec: EP-9), and NULL is not
    -- distinct enough for a primary key to say so, so the unique index below
    -- does.
    prefix     TEXT COLLATE BINARY,
    local_root TEXT NOT NULL,
    -- What the filesystem under `local_root` was when a scan last saw it, in
    -- whatever opaque form the platform could state (spec: EP-12). NULL until a
    -- scan has seen it, and NULL again whenever the mapping is recorded afresh
    -- — which is how a device re-confirms a root a run reported unavailable.
    root_identity TEXT,
    -- The identity this mapping expects the marker in `local_root` to carry,
    -- as the sixteen lowercase hex characters it is spelled in (spec: EP-13).
    -- NULL where the mapping records none, which is a mapping nothing may be
    -- placed through.
    expected_root_id TEXT
) STRICT;

CREATE UNIQUE INDEX mappings_by_prefix ON mappings (ifnull(prefix, ''));

CREATE TABLE pending_rows (
    -- The local provenance that makes cleaning up an uncommitted Container
    -- possible at all (spec: OC-2, OC-3). The row precedes the file it names:
    -- it is written before the spool file is created, so no ciphertext this
    -- device produces is ever unaccounted for, and `state` is what says whether
    -- the file at `spool_path` is a whole Container yet. `materializes` says
    -- whether completing the row marks the Container's Entries present: true
    -- for a Container built out of this device's own files, false for one
    -- rebuilt out of another Container's bytes (spec: OC-7, PK-10).
    container_id BLOB PRIMARY KEY,
    spool_path   TEXT NOT NULL,
    state        TEXT NOT NULL,
    batch        TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    object_ref   TEXT,
    commit_attempted INTEGER NOT NULL DEFAULT 1 CHECK (commit_attempted IN (0, 1)),
    materializes INTEGER NOT NULL DEFAULT 1 CHECK (materializes IN (0, 1))
) STRICT;
"#;

/// The table of materialization records, which layout 17 laid out afresh to
/// name the Entry each record is about.
///
/// Apart from the rest of the device-local group because the upgrade from an
/// older layout lays it out on its own, and refills it (spec: EP-10).
pub(crate) const LOCAL_ENTRIES_DDL: &str = r#"
CREATE TABLE local_entries (
    -- A row outlives the Entry it was made for: a path that leaves the Library
    -- keeps its row, which is what lets the file be reported rather than left
    -- behind unnoticed (spec: EP-10). No foreign key, for that reason. `hash`
    -- is the content hash of the Entry this device last made the file match,
    -- which is what tells a file left as the Library had it from one edited
    -- since, once the Entry itself has left the catalog (spec: EP-15); NULL only
    -- on a row an older layout wrote, which never recorded one. `relocate_to`
    -- is where the Entry this file is a copy of stands now, once a rename has
    -- changed its current path and the file has not followed it, and NULL
    -- otherwise: the row stays keyed by `path`, where the file is
    -- (spec: FM-15, EP-10). `entry_container` and `entry_number` name that
    -- Entry by what no rename changes, kept current as a replacement gives it a
    -- new Container, and the last ones it had once it leaves (spec: FM-9,
    -- EP-10).
    path            TEXT PRIMARY KEY COLLATE BINARY,
    state           TEXT NOT NULL,
    observed_size   INTEGER NOT NULL,
    observed_mtime  INTEGER NOT NULL,
    observed_at     INTEGER NOT NULL,
    hash            BLOB,
    relocate_to     TEXT COLLATE BINARY,
    entry_container BLOB NOT NULL,
    entry_number    INTEGER NOT NULL
) STRICT;

-- A record finds the Entry it is a copy of by its name, and a removal or a
-- rename finds the records of the Entries it reaches the same way.
CREATE INDEX local_entries_by_entry ON local_entries (entry_container, entry_number);
"#;

/// The table of held parcels, which layout 11 added to the device-local group.
///
/// Apart from the rest of that group because the upgrade from an older layout
/// lays it out on its own (spec: PK-21).
pub(crate) const HELD_PARCELS_DDL: &str = r#"
CREATE TABLE held_parcels (
    -- One parcel of one Container whose ciphertext this device keeps at
    -- `file_path`, a local path rather than an Entry Path (spec: PK-21). The
    -- row precedes the file, as a pending row precedes its spool (spec: OC-2),
    -- so a row may name a file that is short or missing, which the fetch meets
    -- as a parcel not held. `plaintext_start` and `plaintext_end` are the
    -- stretch of the Container's plaintext stream the parcel opens into, which
    -- is what says which Entries it covers. No foreign key: a parcel outlives
    -- its Container's leaving the catalog until it is let go.
    container_id    BLOB NOT NULL,
    parcel          INTEGER NOT NULL,
    plaintext_start INTEGER NOT NULL,
    plaintext_end   INTEGER NOT NULL,
    file_path       TEXT NOT NULL,
    PRIMARY KEY (container_id, parcel)
) STRICT;
"#;

/// The table of attempted commit slots, which layout 12 added to the
/// device-local group.
///
/// Apart from the rest of that group because the upgrade from an older layout
/// lays it out on its own (spec: OC-2).
pub(crate) const ATTEMPTED_SLOTS_DDL: &str = r#"
CREATE TABLE attempted_slots (
    -- A commit slot a batch's Journal create was about to consume, written in
    -- the same transaction that marks the batch's pending rows attempted, so
    -- one is never there without the other (spec: OC-2). What a settle reads
    -- back to prove the batch did not commit there (spec: OC-3). `generation`
    -- names the slot `head-<generation>`; `provider_id` is the identifier
    -- Storage minted for it, NULL where the name is the slot (spec: CP-2). No
    -- foreign key: `batch` is not unique in `pending_rows`, and a batch's slots
    -- go with its last row instead.
    batch       TEXT NOT NULL,
    generation  INTEGER NOT NULL,
    provider_id TEXT
) STRICT;

CREATE UNIQUE INDEX attempted_slots_by_batch
    ON attempted_slots (batch, generation, ifnull(provider_id, ''));
"#;

/// The tables of reservations, which layout 18 added to the device-local group
/// and layout 20 gave the mark of a finished drop.
///
/// Apart from the rest of that group because the upgrade from an older layout
/// lays them out on their own (spec: PK-17).
pub(crate) const RESERVATIONS_DDL: &str = r#"
CREATE TABLE reservations (
    -- A drop's choice to add its files as a Pack, written before the first of
    -- them lands (spec: PK-17). `AUTOINCREMENT` so that an identifier is never
    -- drawn twice, a released one's included: the order of `id` is the order
    -- the drops were made in, which is the order a server resumes them in.
    -- `folder` is the folder the drop was made onto, NULL for the Library
    -- root. `received` is set once the drop has stopped receiving with every
    -- file it brought: until then the reservation is a drop in progress, which
    -- no freeze decides and a drop onto the same folder takes over
    -- (spec: PK-17, LA-12).
    id       INTEGER PRIMARY KEY AUTOINCREMENT,
    folder   TEXT COLLATE BINARY,
    received INTEGER NOT NULL DEFAULT 0 CHECK (received IN (0, 1))
) STRICT;

CREATE TABLE reserved_paths (
    -- One Entry Path a reservation names, added before the file the drop
    -- writes there lands.
    reservation INTEGER NOT NULL REFERENCES reservations(id),
    path        TEXT NOT NULL COLLATE BINARY,
    PRIMARY KEY (reservation, path)
) STRICT;
"#;

/// Brings a connection to the layout this build works in.
///
/// A file with no layout at all gets one; a file already at this version is left
/// as it is. An older one is decided by how old it is: down to
/// [`DEVICE_SCHEMA_VERSION`] its device-local group is one this build reads, so
/// the catalog alone would be discarded and the next catch-up rebuild it from
/// Storage (spec: RV-5); in this build that window holds layout 20 alone, which
/// is met first by an upgrade of its own that keeps its catalog and gives the
/// checkpoint row the Library's name. Below the floor, layouts 19 and 18 gain
/// the mark of a finished drop on their reservations, the newer of the two
/// gaining the name as layout 20 does and the older losing its catalog,
/// layout 17 gains the tables of reservations and layout 16 has its
/// materialization records tied to the Entries its catalog holds, and those two
/// then lose their catalog the same way; layouts 7 to 15 have their device-local
/// group upgraded in place and lose their catalog too. Anything else — older
/// than layout 7, or written by a build that came after this one — is refused,
/// because guessing at a layout this build does not know is how a cache turns
/// into wrong answers.
pub(crate) fn prepare(connection: &mut Connection) -> IndexResult<()> {
    // An Entry may only name a Container the catalog holds, and SQLite enforces
    // that only when asked to — per connection, not per file.
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(classify("enabling foreign keys"))?;

    match stamp(connection)? {
        0 => create(connection),
        SCHEMA_VERSION => Ok(()),
        20 => library_name::add(connection),
        18 | 19 => reservations::add_the_received_mark(connection),
        17 => reservations::upgrade(connection),
        16 => entry_identities::upgrade(connection),
        7..=15 => provenance::preserve_device_state(connection),
        found if carries_a_readable_device_group(found) => discard_the_catalog(connection),
        found => Err(unsupported(found)),
    }
}

/// Whether an older file's device-local group is one this build still reads.
fn carries_a_readable_device_group(found: i64) -> bool {
    within(DEVICE_SCHEMA_VERSION..SCHEMA_VERSION, found)
}

/// The question above with the window handed in rather than read off the two
/// constants.
///
/// Split out so the rule can be stated against a synthetic pair of versions: in
/// this build the real window holds no layout, every older one taking an
/// upgrade instead, so a case over the real constants cannot show where its
/// two edges fall.
fn within(window: Range<i64>, found: i64) -> bool {
    window.contains(&found)
}

/// The version stamped into the file, and 0 where nothing has stamped one.
fn stamp(connection: &Connection) -> IndexResult<i64> {
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(classify("reading the Index file's schema version"))
}

/// Lays out both groups in a file that has neither.
fn create(connection: &Connection) -> IndexResult<()> {
    const OPERATION: &str = "creating the Index schema";

    connection
        .execute_batch(LIBRARY_WIDE_DDL)
        .map_err(classify(OPERATION))?;
    connection
        .execute_batch(DEVICE_LOCAL_DDL)
        .map_err(classify(OPERATION))?;
    connection
        .execute_batch(LOCAL_ENTRIES_DDL)
        .map_err(classify(OPERATION))?;
    connection
        .execute_batch(HELD_PARCELS_DDL)
        .map_err(classify(OPERATION))?;
    connection
        .execute_batch(ATTEMPTED_SLOTS_DDL)
        .map_err(classify(OPERATION))?;
    connection
        .execute_batch(RESERVATIONS_DDL)
        .map_err(classify(OPERATION))?;
    connection
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(classify("stamping the Index file's schema version"))
}

/// Throws the catalog of an older layout away and lays a current one out in its
/// place, leaving this device's own state where it is.
///
/// One `BEGIN IMMEDIATE` covers the whole of it, from re-reading the stamp to
/// writing the new one, because two processes opening the same file at once is
/// the ordinary arrangement rather than a mistake: whichever takes the write
/// lock second reads the stamp the first one left, finds the layout already
/// current, and does nothing. A crash part-way leaves the old file exactly as it
/// was, and the next open discards it again.
fn discard_the_catalog(connection: &mut Connection) -> IndexResult<()> {
    // What goes in the WARN below: a summary of the whole discard, not the
    // statement that was running when one step of it failed. Each
    // `map_err` further down names that statement on its own, the way every
    // other write in this crate does — so a `Backend` failure here says which
    // one of five ran into it, not just that "the discard" did.
    const SUMMARY: &str = "discarding the catalog of an older Index layout";

    let transaction = connection
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(classify("beginning the catalog discard"))?;

    let found = match stamp(&transaction)? {
        // Another connection got here first, and there is nothing left to do —
        // least of all a second discard of a catalog it has already rebuilt.
        SCHEMA_VERSION => return Ok(()),
        found if carries_a_readable_device_group(found) => found,
        found => return Err(unsupported(found)),
    };

    replace_the_catalog(&transaction)?;
    transaction
        .commit()
        .map_err(classify("committing the discarded catalog"))?;
    report_the_discard(SUMMARY, found);
    Ok(())
}

/// Drops an older layout's catalog and lays a current, empty one out in its
/// place, stamping the file as the current layout — inside the caller's
/// transaction, which is what keeps a crash from leaving half of it done.
pub(super) fn replace_the_catalog(transaction: &Connection) -> IndexResult<()> {
    // The Entries before the Containers they refer to, and the checkpoint last:
    // foreign keys are on for this connection, so a parent dropped out from
    // under its children is a violation rather than a cascade.
    transaction
        .execute_batch("DROP TABLE entries; DROP TABLE containers; DROP TABLE checkpoint;")
        .map_err(classify("dropping the catalog of an older Index layout"))?;
    transaction
        .execute_batch(LIBRARY_WIDE_DDL)
        .map_err(classify("recreating the Index catalog"))?;
    transaction
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(classify("stamping the Index file's schema version"))
}

/// Says that a catalog was thrown away, once the transaction that did it has
/// committed.
///
/// The two versions and what was done with them, and nothing that names a
/// file: the Index lives under the state directory and its path is the owner's
/// own, which is one of the things an event may never carry.
pub(super) fn report_the_discard(operation: &'static str, found: i64) {
    tracing::warn!(
        operation,
        found,
        supported = SCHEMA_VERSION,
        "the Index file held a catalog of an older layout; it has been discarded, and the \
         next catch-up rebuilds it from Storage"
    );
}

/// A file this build can neither read nor carry forward.
fn unsupported(found: i64) -> IndexError {
    IndexError::UnsupportedSchema {
        found,
        supported: SCHEMA_VERSION,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where the two edges of the window fall, over a pair of versions this
    /// build does not carry.
    ///
    /// The rule is that a file stamped at or above the device-local floor and
    /// below the current layout keeps its device group and loses its catalog,
    /// and that everything outside is not discarded this way. In this build the
    /// real window holds no layout, every older one taking an upgrade instead,
    /// so this is the one place the rule itself is stated —
    /// with numbers of its own, the way the suites beside this crate write
    /// their layout versions out rather than reading them off the constants.
    #[test]
    fn the_window_takes_the_floor_and_stops_below_the_current_layout() {
        let window = 4..6;

        assert!(within(window.clone(), 4), "the floor is inside the window");
        assert!(within(window.clone(), 5), "and so is everything under it");
        assert!(
            !within(window.clone(), 6),
            "the current layout is not an older one to discard the catalog of"
        );
        assert!(
            !within(window.clone(), 3),
            "below the floor there is no device group left to keep"
        );
        assert!(
            !within(window, 99),
            "and a layout from a later build is not readable at all"
        );
    }

    /// This build's own window holds layout 20 alone: layout 21 changed the
    /// Library-wide group and nothing else, so a layout-20 file's device group
    /// is this build's — and `prepare` meets it first with the upgrade that
    /// keeps its catalog too and gives the checkpoint row the Library's name,
    /// while every layout from 7 to 19 takes an upgrade of its own
    /// (spec: OC-2, EP-10, PK-17, FM-15).
    #[test]
    fn this_builds_window_holds_the_layout_before_the_library_name() {
        assert_eq!(DEVICE_SCHEMA_VERSION, 20);
        assert_eq!(SCHEMA_VERSION, 21);
        for found in 0..=SCHEMA_VERSION + 1 {
            assert_eq!(
                carries_a_readable_device_group(found),
                found == 20,
                "layout {found}"
            );
        }
    }
}
