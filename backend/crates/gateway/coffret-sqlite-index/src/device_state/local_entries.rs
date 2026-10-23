use coffret_model::{EntryLocation, EntryPath};
use coffret_usecase::device_state::{DeviceTime, LocalEntry, LocalEntryState, LocalObservation};
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::error::classify;
use crate::path_prefix::subtree_range;
use crate::query::{collect, first};
use crate::rows;

/// Records that this device now has a file on disk, as a copy of the Entry
/// the observation names (spec: EP-10).
///
/// The row then waits for whatever that Entry's current path asks of it, which
/// [`reconcile`] reads off the catalog in the same transaction.
pub(crate) fn mark_present(
    connection: &Connection,
    observation: &LocalObservation,
) -> IndexResult<()> {
    const OPERATION: &str = "recording a materialized file";
    connection
        .execute(
            "INSERT INTO local_entries (
                 path, state, observed_size, observed_mtime, observed_at, hash, relocate_to,
                 entry_container, entry_number
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8)
             ON CONFLICT (path) DO UPDATE SET
                 state = excluded.state,
                 observed_size = excluded.observed_size,
                 observed_mtime = excluded.observed_mtime,
                 observed_at = excluded.observed_at,
                 hash = excluded.hash,
                 relocate_to = NULL,
                 entry_container = excluded.entry_container,
                 entry_number = excluded.entry_number",
            params![
                observation.path.as_str(),
                rows::state_text(LocalEntryState::Present),
                rows::to_integer(OPERATION, "observed_size", observation.size)?,
                observation.mtime.as_unix_seconds(),
                observation.at.as_unix_seconds(),
                observation
                    .hash
                    .as_ref()
                    .map(|hash| hash.as_bytes().as_slice()),
                observation.entry.container_id.as_bytes().as_slice(),
                observation.entry.entry_number.get(),
            ],
        )
        .map_err(classify(OPERATION))?;
    reconcile(connection, [observation.path.as_str()], OPERATION)
}

/// Every row whose file waits to follow its Entry elsewhere (spec: EP-10).
pub(crate) fn awaiting_relocation(connection: &Connection) -> IndexResult<Vec<LocalEntry>> {
    collect(
        connection,
        "SELECT * FROM local_entries WHERE relocate_to IS NOT NULL ORDER BY path",
        [],
        "reading what waits to be relocated",
        rows::local_entry,
    )
}

/// Re-keys rows to the paths their files now stand at, every row a relocation names
/// taken out before any is put back (spec: EP-10).
pub(crate) fn relocate_local_entries(
    connection: &Connection,
    relocated: &[(EntryPath, EntryPath)],
) -> IndexResult<()> {
    const OPERATION: &str = "re-keying relocated files' rows";
    let mut taken = Vec::with_capacity(relocated.len());
    for (from, to) in relocated {
        if let Some(local) = local_entry_at(connection, from)? {
            taken.push((local, to));
        }
    }
    for (local, _) in &taken {
        forget_local_entry(connection, &local.observation.path)?;
    }
    for (local, to) in &taken {
        connection
            .execute(
                "INSERT OR REPLACE INTO local_entries (
                     path, state, observed_size, observed_mtime, observed_at, hash, relocate_to,
                     entry_container, entry_number
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7, ?8)",
                params![
                    to.as_str(),
                    rows::state_text(local.state),
                    rows::to_integer(OPERATION, "observed_size", local.observation.size)?,
                    local.observation.mtime.as_unix_seconds(),
                    local.observation.at.as_unix_seconds(),
                    local
                        .observation
                        .hash
                        .as_ref()
                        .map(|hash| hash.as_bytes().as_slice()),
                    local.observation.entry.container_id.as_bytes().as_slice(),
                    local.observation.entry.entry_number.get(),
                ],
            )
            .map_err(classify(OPERATION))?;
    }
    reconcile(
        connection,
        taken.iter().map(|(_, to)| to.as_str()),
        OPERATION,
    )
}

/// Brings the rows at `paths` in line with where the catalog holds the Entry
/// each of them is a copy of (spec: EP-10, FM-15).
///
/// A row whose Entry is current waits to follow it where it stands at another
/// path, and waits for nothing where it stands at the row's own. A row whose
/// Entry has left the Library waits for nothing either, and where no current
/// Entry stands at its path it stays, and the file is departed there
/// (spec: EP-15). Where one does, it is another Entry than the one the file is
/// a copy of, so the row is forgotten: a record kept there would claim that
/// Entry present and offer this file's bytes as its edit. Forgotten, the file
/// is one this device never placed for it (spec: EP-11).
pub(crate) fn reconcile<'p>(
    connection: &Connection,
    paths: impl IntoIterator<Item = &'p str>,
    operation: &'static str,
) -> IndexResult<()> {
    let mut follow = connection
        .prepare_cached(
            "UPDATE local_entries SET relocate_to = (
                 SELECT NULLIF(entries.path, local_entries.path) FROM entries
                 WHERE entries.container_id = local_entries.entry_container
                 AND entries.entry_number = local_entries.entry_number
             ) WHERE path = ?1",
        )
        .map_err(classify(operation))?;
    let mut forget = connection
        .prepare_cached(
            "DELETE FROM local_entries WHERE path = ?1
             AND EXISTS (SELECT 1 FROM entries WHERE entries.path = ?1)
             AND NOT EXISTS (
                 SELECT 1 FROM entries
                 WHERE entries.container_id = local_entries.entry_container
                 AND entries.entry_number = local_entries.entry_number
             )",
        )
        .map_err(classify(operation))?;
    for path in paths {
        follow.execute(params![path]).map_err(classify(operation))?;
        forget.execute(params![path]).map_err(classify(operation))?;
    }
    Ok(())
}

/// [`reconcile`] over every row this device holds.
pub(crate) fn reconcile_all(connection: &Connection, operation: &'static str) -> IndexResult<()> {
    connection
        .execute_batch(
            "UPDATE local_entries SET relocate_to = (
                 SELECT NULLIF(entries.path, local_entries.path) FROM entries
                 WHERE entries.container_id = local_entries.entry_container
                 AND entries.entry_number = local_entries.entry_number
             );
             DELETE FROM local_entries
             WHERE EXISTS (SELECT 1 FROM entries WHERE entries.path = local_entries.path)
             AND NOT EXISTS (
                 SELECT 1 FROM entries
                 WHERE entries.container_id = local_entries.entry_container
                 AND entries.entry_number = local_entries.entry_number
             );",
        )
        .map_err(classify(operation))
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
        .map_err(classify("recording a file as gone"))?;
    Ok(())
}

/// Forgets this device's row for one Entry Path, so the path is outside its
/// scope again (spec: EP-10, EP-15).
///
/// A `DELETE`, so a path with no row is left as it is: an interrupted run's
/// next pass forgets it again and finds nothing to forget.
pub(crate) fn forget_local_entry(connection: &Connection, path: &EntryPath) -> IndexResult<()> {
    connection
        .execute(
            "DELETE FROM local_entries WHERE path = ?1",
            params![path.as_str()],
        )
        .map_err(classify("forgetting a local file's row"))?;
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

/// The files this device has on disk under a prefix (spec: EP-9, EP-10), as
/// copies of the Entries at their own paths: a file waiting to follow a rename
/// elsewhere is not one (spec: FM-15).
pub(crate) fn present_under(
    connection: &Connection,
    prefix: Option<&EntryPath>,
) -> IndexResult<Vec<LocalEntry>> {
    const OPERATION: &str = "reading what this device has";
    let present = rows::state_text(LocalEntryState::Present);
    match prefix {
        None => collect(
            connection,
            "SELECT * FROM local_entries \
             WHERE state = ?1 AND relocate_to IS NULL \
             ORDER BY path",
            params![present],
            OPERATION,
            rows::local_entry,
        ),
        Some(prefix) => {
            let (lower, upper) = subtree_range(prefix);
            collect(
                connection,
                "SELECT * FROM local_entries \
                 WHERE state = ?1 AND relocate_to IS NULL \
                 AND (path = ?2 OR (path >= ?3 AND path < ?4)) \
                 ORDER BY path",
                params![present, prefix.as_str(), lower, upper],
                OPERATION,
                rows::local_entry,
            )
        }
    }
}

/// Every current Entry under a prefix that this device does not have on disk,
/// ordered by Entry Path bytes: the catalog's Entries joined against the rows
/// [`present_under`] reads, in one statement.
pub(crate) fn entries_absent_under(
    connection: &Connection,
    prefix: Option<&EntryPath>,
) -> IndexResult<Vec<EntryLocation>> {
    const OPERATION: &str = "reading a subtree's Entries this device does not have";
    let present = rows::state_text(LocalEntryState::Present);
    match prefix {
        None => collect(
            connection,
            "SELECT entries.* FROM entries \
             LEFT JOIN local_entries ON local_entries.path = entries.path \
             AND local_entries.state = ?1 AND local_entries.relocate_to IS NULL \
             WHERE local_entries.path IS NULL \
             ORDER BY entries.path",
            params![present],
            OPERATION,
            rows::entry_location,
        ),
        Some(prefix) => {
            let (lower, upper) = subtree_range(prefix);
            collect(
                connection,
                "SELECT entries.* FROM entries \
                 LEFT JOIN local_entries ON local_entries.path = entries.path \
                 AND local_entries.state = ?1 AND local_entries.relocate_to IS NULL \
                 WHERE local_entries.path IS NULL \
                 AND (entries.path = ?2 OR (entries.path >= ?3 AND entries.path < ?4)) \
                 ORDER BY entries.path",
                params![present, prefix.as_str(), lower, upper],
                OPERATION,
                rows::entry_location,
            )
        }
    }
}

/// The files this device has at paths the Library holds no current Entry for
/// (spec: EP-10), leaving out a file waiting to follow a rename of its Entry
/// elsewhere: its Entry has not left the Library (spec: FM-15).
pub(crate) fn present_without_entry(connection: &Connection) -> IndexResult<Vec<LocalEntry>> {
    collect(
        connection,
        "SELECT local_entries.* FROM local_entries \
         LEFT JOIN entries ON entries.path = local_entries.path \
         WHERE local_entries.state = ?1 AND local_entries.relocate_to IS NULL \
         AND entries.path IS NULL \
         ORDER BY local_entries.path",
        params![rows::state_text(LocalEntryState::Present)],
        "reading what the Library left behind",
        rows::local_entry,
    )
}
