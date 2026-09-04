use coffret_usecase::{IndexError, IndexResult};
use rusqlite::{Connection, TransactionBehavior};

use crate::error::translate;

/// The layout this build writes and reads.
///
/// There is no migration path and deliberately so: the catalog is a cache that
/// can be rebuilt exactly from Storage (spec: RV-5), so a catalog this build
/// does not understand is discarded rather than converted, and the conversion
/// code that would otherwise have to be right for every past layout never has to
/// exist.
///
/// Every change to either group below moves this, however small, and so does
/// every change to the vocabulary a column's values are spelled in: the texts a
/// state is stored as belong to the layout as much as the columns holding them
/// do. A file stamped with the version this build carries is opened untouched,
/// so a change that left the number alone would open a file this build misreads
/// — a query over a column that is not there, or a stored text no match arm
/// knows — and fail with a backend error saying nothing about why.
pub(crate) const SCHEMA_VERSION: i64 = 5;

/// The version the device-local group last changed at.
///
/// Discarding is the right answer only for the half of the file that is a
/// cache. The device-local group is not one: no Snapshot carries it and no
/// catch-up rebuilds it (spec: EP-9, EP-10, OC-2), so where this device maps
/// the Library and which spool an interrupted run left behind exist nowhere but
/// in this file. Throwing that away to be rid of a stale catalog would cost the
/// owner the one record of it and gain nothing that Storage was not about to
/// hand back anyway.
///
/// So a file stamped anywhere in `DEVICE_SCHEMA_VERSION..SCHEMA_VERSION` is one
/// whose device-local group this build still reads: its catalog alone is
/// discarded and the rest is left as it is. Below that, the file is refused
/// whole, because there is no group left in it worth opening it for.
///
/// Every change to a device-local table, or to the vocabulary its values are
/// spelled in, moves this to the new [`SCHEMA_VERSION`]; a change confined to
/// the Library-wide group leaves it where it is.
///
/// **The two columns every layout keeps.** Below this floor even `mappings` is
/// not read, but `prefix` and `local_root` are exempt from the rule that a
/// refused file is opened for nothing: they have named exactly what they name
/// since layout 1, and every layout to come keeps them so, whatever else about
/// the table changes. They are the one piece of this device's own state that
/// cannot be recreated from memory, so [`RefusedIndex`](crate::RefusedIndex)
/// may still read a refused file for them alone — by column name, with no
/// layout check and no write — which is what lets a refusal's own recovery be
/// more than "the one record of where your Library lives is gone with the
/// file".
pub(crate) const DEVICE_SCHEMA_VERSION: i64 = 4;

/// The group an Index Snapshot carries: the whole Library, identical on every
/// enrolled device (spec: CK-7).
///
/// It stands on its own so that it can be laid out on its own, which is what a
/// discard does with it — the boundary between the two groups is a real one
/// rather than a comment somebody has to honour.
///
/// Every Entry Path is stored in its canonical form (spec: EP-1, EP-2) and
/// compared bytewise: `BINARY` is spelled out on each path column.
const LIBRARY_WIDE_DDL: &str = r#"
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
    adopted_snapshot      TEXT
) STRICT;

CREATE TABLE containers (
    id              BLOB PRIMARY KEY,
    kind            TEXT NOT NULL,
    ciphertext_hash BLOB NOT NULL,
    ciphertext_len  INTEGER NOT NULL,
    object_ref      TEXT
) STRICT;

CREATE TABLE entries (
    -- UNIQUE by being the primary key: a defence, not the enforcement. What
    -- keeps one Entry Path to one current Entry is the commit's own check
    -- (spec: EP-5, EP-6); this catches a record that reached the catalog
    -- claiming otherwise.
    path                   TEXT PRIMARY KEY COLLATE BINARY,
    container_id           BLOB NOT NULL REFERENCES containers(id),
    "offset"               INTEGER NOT NULL,
    size                   INTEGER NOT NULL,
    mtime                  INTEGER NOT NULL,
    -- NULL where the platform that wrote the Container reported no birth time,
    -- which is the whole of what absent means (spec: FM-9, FM-15).
    btime                  INTEGER,
    hash                   BLOB NOT NULL,
    mime                   TEXT,
    derived_from_container BLOB,
    derived_from_path      TEXT COLLATE BINARY
) STRICT;

-- Removing a Container removes its Entries, and a Pack holds many.
CREATE INDEX entries_by_container ON entries (container_id);
"#;

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
    root_identity TEXT
) STRICT;

CREATE UNIQUE INDEX mappings_by_prefix ON mappings (ifnull(prefix, ''));

CREATE TABLE local_entries (
    -- A row outlives the Entry it was made for: a path that leaves the Library
    -- keeps its row, which is what lets the file be reported rather than left
    -- behind unnoticed (spec: EP-10). No foreign key, for that reason.
    path           TEXT PRIMARY KEY COLLATE BINARY,
    state          TEXT NOT NULL,
    observed_size  INTEGER NOT NULL,
    observed_mtime INTEGER NOT NULL,
    observed_at    INTEGER NOT NULL
) STRICT;

CREATE TABLE pending_uploads (
    -- The local provenance that makes cleaning up an uncommitted Container
    -- possible at all (spec: OC-2, OC-3). The row precedes the file it names:
    -- it is written before the spool file is created, so no ciphertext this
    -- device produces is ever unaccounted for, and `state` is what says whether
    -- the file at `spool_path` is a whole Container yet.
    container_id BLOB PRIMARY KEY,
    spool_path   TEXT NOT NULL,
    state        TEXT NOT NULL,
    batch        TEXT NOT NULL,
    created_at   INTEGER NOT NULL,
    object_ref   TEXT
) STRICT;
"#;

/// Brings a connection to the layout this build works in.
///
/// A file with no layout at all gets one; a file already at this version is left
/// as it is. An older one is decided by how old it is: down to
/// [`DEVICE_SCHEMA_VERSION`] its device-local group is one this build reads, so
/// the catalog alone is discarded and the next catch-up rebuilds it from Storage
/// (spec: RV-5). Anything else — older than that, or written by a build that
/// came after this one — is refused, because guessing at a layout this build
/// does not know is how a cache turns into wrong answers.
pub(crate) fn prepare(connection: &mut Connection) -> IndexResult<()> {
    // An Entry may only name a Container the catalog holds, and SQLite enforces
    // that only when asked to — per connection, not per file.
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(translate("enabling foreign keys"))?;

    match stamp(connection)? {
        0 => create(connection),
        SCHEMA_VERSION => Ok(()),
        found if carries_a_readable_device_group(found) => discard_the_catalog(connection),
        found => Err(unsupported(found)),
    }
}

/// Whether an older file's device-local group is one this build still reads.
fn carries_a_readable_device_group(found: i64) -> bool {
    (DEVICE_SCHEMA_VERSION..SCHEMA_VERSION).contains(&found)
}

/// The version stamped into the file, and 0 where nothing has stamped one.
fn stamp(connection: &Connection) -> IndexResult<i64> {
    connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(translate("reading the Index file's schema version"))
}

/// Lays out both groups in a file that has neither.
fn create(connection: &Connection) -> IndexResult<()> {
    const OPERATION: &str = "creating the Index schema";

    connection
        .execute_batch(LIBRARY_WIDE_DDL)
        .map_err(translate(OPERATION))?;
    connection
        .execute_batch(DEVICE_LOCAL_DDL)
        .map_err(translate(OPERATION))?;
    connection
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(translate("stamping the Index file's schema version"))
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
        .map_err(translate("beginning the catalog discard"))?;

    let found = match stamp(&transaction)? {
        // Another connection got here first, and there is nothing left to do —
        // least of all a second discard of a catalog it has already rebuilt.
        SCHEMA_VERSION => return Ok(()),
        found if carries_a_readable_device_group(found) => found,
        found => return Err(unsupported(found)),
    };

    // The Entries before the Containers they refer to, and the checkpoint last:
    // foreign keys are on for this connection, so a parent dropped out from
    // under its children is a violation rather than a cascade.
    transaction
        .execute_batch("DROP TABLE entries; DROP TABLE containers; DROP TABLE checkpoint;")
        .map_err(translate("dropping the catalog of an older Index layout"))?;
    transaction
        .execute_batch(LIBRARY_WIDE_DDL)
        .map_err(translate("recreating the Index catalog"))?;
    transaction
        .pragma_update(None, "user_version", SCHEMA_VERSION)
        .map_err(translate("stamping the Index file's schema version"))?;
    transaction
        .commit()
        .map_err(translate("committing the discarded catalog"))?;

    // The two versions and what was done with them, and nothing that names a
    // file: the Index lives under the state directory and its path is the
    // owner's own, which is one of the things an event may never carry.
    tracing::warn!(
        operation = SUMMARY,
        found,
        supported = SCHEMA_VERSION,
        "the Index file held a catalog of an older layout; it has been discarded, and the \
         next catch-up rebuilds it from Storage"
    );
    Ok(())
}

/// A file this build can neither read nor carry forward.
fn unsupported(found: i64) -> IndexError {
    IndexError::UnsupportedSchema {
        found,
        supported: SCHEMA_VERSION,
    }
}
