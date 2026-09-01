use coffret_usecase::{IndexError, IndexResult};
use rusqlite::Connection;

use crate::error::translate;

/// The layout this build writes and reads.
///
/// There is no migration path and deliberately so: the Index is a cache that
/// can be rebuilt exactly from Storage (spec: RV-5), so a file this build does
/// not understand is discarded rather than converted, and the conversion code
/// that would otherwise have to be right for every past layout never has to
/// exist.
///
/// Every change to [`DDL`] moves this, however small, and so does every change
/// to the vocabulary a column's values are spelled in: the texts a state is
/// stored as belong to the layout as much as the columns holding them do. A
/// file stamped with the version this build carries is opened untouched, so a
/// change that left the number alone would open a file this build misreads — a
/// query over a column that is not there, or a stored text no match arm knows —
/// and fail with a backend error saying nothing about why.
pub(crate) const SCHEMA_VERSION: i64 = 5;

/// The two groups of tables.
///
/// Everything above the divider is what an Index Snapshot carries: the whole
/// Library, identical on every enrolled device (spec: CK-7). Everything below
/// it is this device's alone and is never uploaded (spec: EP-9, EP-10, OC-2).
///
/// Every Entry Path is stored in its canonical form (spec: EP-1, EP-2) and
/// compared bytewise: `BINARY` is spelled out on each path column.
const DDL: &str = r#"
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

-- Device-local from here down. Never in a Snapshot (spec: CK-7).

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
/// A file with no layout at all gets one; a file already at this version is
/// left as it is; anything else is refused, because guessing at a layout this
/// build does not know is how a cache turns into wrong answers.
pub(crate) fn prepare(connection: &Connection) -> IndexResult<()> {
    // An Entry may only name a Container the catalog holds, and SQLite enforces
    // that only when asked to — per connection, not per file.
    connection
        .pragma_update(None, "foreign_keys", true)
        .map_err(translate("enabling foreign keys"))?;

    let found: i64 = connection
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(translate("reading the Index file's schema version"))?;

    match found {
        0 => {
            connection
                .execute_batch(DDL)
                .map_err(translate("creating the Index schema"))?;
            connection
                .pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(translate("stamping the Index file's schema version"))?;
            Ok(())
        }
        SCHEMA_VERSION => Ok(()),
        found => Err(IndexError::UnsupportedSchema {
            found,
            supported: SCHEMA_VERSION,
        }),
    }
}
