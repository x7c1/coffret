//! The three tables that never leave this device.
//!
//! Nothing here is ever part of an Index Snapshot (spec: CK-7), and nothing the
//! Library-wide operations do reaches these tables.

use coffret_model::{ContainerId, EntryPath, ObjectRef};
use coffret_usecase::device_state::{
    DeviceTime, LocalEntry, LocalEntryState, LocalObservation, Mapping, PendingSpoolState,
    PendingUpload,
};
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::error::{path_text, translate};
use crate::path_prefix::subtree_range;
use crate::query::{collect, first};
use crate::rows;

/// Records where one part of the Library lives on this device (spec: EP-9).
///
/// `IS` rather than `=` because the Library root is stored as NULL, and NULL is
/// equal to nothing, itself included.
pub(crate) fn set_mapping(connection: &Connection, mapping: &Mapping) -> IndexResult<()> {
    const OPERATION: &str = "recording a mapping";
    let prefix = mapping.prefix.as_ref().map(EntryPath::as_str);
    let local_root = path_text(&mapping.local_root, OPERATION)?;

    connection
        .execute("DELETE FROM mappings WHERE prefix IS ?1", params![prefix])
        .map_err(translate(OPERATION))?;
    connection
        .execute(
            "INSERT INTO mappings (prefix, local_root) VALUES (?1, ?2)",
            params![prefix, local_root],
        )
        .map_err(translate(OPERATION))?;
    Ok(())
}

/// Every mapping this device holds, the Library root first (spec: EP-9).
pub(crate) fn mappings(connection: &Connection) -> IndexResult<Vec<Mapping>> {
    collect(
        connection,
        // `prefix IS NOT NULL` is 0 for the root mapping and 1 for the rest, so
        // the root sorts ahead of the subtrees it stands in for.
        "SELECT * FROM mappings ORDER BY prefix IS NOT NULL, prefix",
        [],
        "reading the mappings",
        rows::mapping,
    )
}

/// Records that this device now has a file on disk (spec: EP-10).
pub(crate) fn mark_present(
    connection: &Connection,
    observation: &LocalObservation,
) -> IndexResult<()> {
    connection
        .execute(
            "INSERT INTO local_entries (path, state, observed_size, observed_mtime, observed_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (path) DO UPDATE SET
                 state = excluded.state,
                 observed_size = excluded.observed_size,
                 observed_mtime = excluded.observed_mtime,
                 observed_at = excluded.observed_at",
            params![
                observation.path.as_str(),
                rows::state_text(LocalEntryState::Present),
                rows::to_integer(observation.size),
                observation.mtime.as_unix_seconds(),
                observation.at.as_unix_seconds(),
            ],
        )
        .map_err(translate("recording a materialized file"))?;
    Ok(())
}

/// Records that a file this device had is gone (spec: EP-10).
///
/// An `UPDATE` and not an upsert, deliberately: a path with no row is one this
/// device never materialized, and giving it one would let the scan report a
/// deletion the device never witnessed. The last observation stays — it is what
/// the file looked like while the device still had it — and only the state and
/// the time of looking move.
pub(crate) fn mark_absent(
    connection: &Connection,
    path: &EntryPath,
    at: DeviceTime,
) -> IndexResult<()> {
    connection
        .execute(
            "UPDATE local_entries SET state = ?2, observed_at = ?3 WHERE path = ?1",
            params![
                path.as_str(),
                rows::state_text(LocalEntryState::Absent),
                at.as_unix_seconds(),
            ],
        )
        .map_err(translate("recording a file as gone"))?;
    Ok(())
}

/// What this device knows about the local file at one Entry Path (spec: EP-10).
pub(crate) fn local_entry_at(
    connection: &Connection,
    path: &EntryPath,
) -> IndexResult<Option<LocalEntry>> {
    first(
        connection,
        "SELECT * FROM local_entries WHERE path = ?1",
        params![path.as_str()],
        "reading a local file's row",
        rows::local_entry,
    )
}

/// The files this device has on disk under a prefix (spec: EP-9, EP-10).
pub(crate) fn present_under(
    connection: &Connection,
    prefix: Option<&EntryPath>,
) -> IndexResult<Vec<LocalEntry>> {
    const OPERATION: &str = "reading what this device has";
    let present = rows::state_text(LocalEntryState::Present);
    match prefix {
        None => collect(
            connection,
            "SELECT * FROM local_entries WHERE state = ?1 ORDER BY path",
            params![present],
            OPERATION,
            rows::local_entry,
        ),
        Some(prefix) => {
            let (lower, upper) = subtree_range(prefix);
            collect(
                connection,
                "SELECT * FROM local_entries \
                 WHERE state = ?1 AND (path = ?2 OR (path >= ?3 AND path < ?4)) \
                 ORDER BY path",
                params![present, prefix.as_str(), lower, upper],
                OPERATION,
                rows::local_entry,
            )
        }
    }
}

/// The files this device has at paths the Library holds no current Entry for
/// (spec: EP-10).
pub(crate) fn present_without_entry(connection: &Connection) -> IndexResult<Vec<LocalEntry>> {
    collect(
        connection,
        "SELECT local_entries.* FROM local_entries \
         LEFT JOIN entries ON entries.path = local_entries.path \
         WHERE local_entries.state = ?1 AND entries.path IS NULL \
         ORDER BY local_entries.path",
        params![rows::state_text(LocalEntryState::Present)],
        "reading what the Library left behind",
        rows::local_entry,
    )
}

/// Records a Container this device is about to spool, has spooled, or has
/// uploaded before its batch committed (spec: OC-2).
pub(crate) fn record_pending_upload(
    connection: &Connection,
    pending: &PendingUpload,
) -> IndexResult<()> {
    const OPERATION: &str = "recording a spool";
    let spool_path = path_text(&pending.spool_path, OPERATION)?;

    connection
        .execute(
            "INSERT INTO pending_uploads \
                 (container_id, spool_path, state, batch, created_at, object_ref)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (container_id) DO UPDATE SET
                 spool_path = excluded.spool_path,
                 state = excluded.state,
                 batch = excluded.batch,
                 created_at = excluded.created_at,
                 object_ref = excluded.object_ref",
            params![
                pending.container_id.as_bytes().as_slice(),
                spool_path,
                rows::spool_state_text(pending.state),
                pending.batch.as_str(),
                pending.created_at.as_unix_seconds(),
                pending.object_ref.as_ref().map(ObjectRef::as_str),
            ],
        )
        .map_err(translate(OPERATION))?;
    Ok(())
}

/// Records that one Container's spool file is complete (spec: OC-2).
///
/// An `UPDATE` and not an upsert, for the reason [`mark_absent`] is one: a
/// Container with no row is one no spool step of this device ever announced, and
/// giving it one would record a spool that does not exist and set a later run
/// looking for a file nobody wrote. Everything the announcing row said about the
/// Container stays — the path, the batch, the moment it was announced — because
/// none of it changed when the file did.
pub(crate) fn complete_pending_spool(
    connection: &Connection,
    container_id: ContainerId,
) -> IndexResult<()> {
    connection
        .execute(
            "UPDATE pending_uploads SET state = ?2 WHERE container_id = ?1",
            params![
                container_id.as_bytes().as_slice(),
                rows::spool_state_text(PendingSpoolState::Written),
            ],
        )
        .map_err(translate("completing a spool"))?;
    Ok(())
}

/// Drops the spool row for one Container, its batch having settled.
///
/// Dropping one that is not there is a no-op, so an interrupted cleanup is
/// simply run again (spec: OC-6).
pub(crate) fn clear_pending_upload(
    connection: &Connection,
    container_id: ContainerId,
) -> IndexResult<()> {
    connection
        .execute(
            "DELETE FROM pending_uploads WHERE container_id = ?1",
            params![container_id.as_bytes().as_slice()],
        )
        .map_err(translate("clearing a spool"))?;
    Ok(())
}

/// Every Container this device is about to spool, has spooled, or has uploaded
/// whose batch has not committed (spec: OC-2).
///
/// `state` comes back with each row, which is what tells a spool this device only
/// announced from one it finished writing.
pub(crate) fn pending_uploads(connection: &Connection) -> IndexResult<Vec<PendingUpload>> {
    collect(
        connection,
        "SELECT * FROM pending_uploads ORDER BY container_id",
        [],
        "reading the spools",
        rows::pending_upload,
    )
}
