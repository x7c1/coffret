use coffret_model::ContainerId;
use coffret_usecase::catalog_renames::RenamedEntry;
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::error::classify;
use crate::query::{collect, first};

/// Marks the rows of the files this device has of renamed Entries as waiting
/// to follow them (spec: EP-10, FM-15).
///
/// A file stays where it was placed, and so does its row. A row follows the
/// Entry it is a copy of: the Entry at its own path where it follows no
/// earlier rename, or the Entry an earlier rename sent it after. Every rename
/// is matched against the rows as they were before any of them was updated, so
/// two Entries swapping their names swap what their two files wait for, and a
/// file whose Entry is renamed back to where it stands waits for nothing any
/// more.
pub(crate) fn follow_renames(connection: &Connection, renamed: &[RenamedEntry]) -> IndexResult<()> {
    const OPERATION: &str = "marking local files to follow a rename";
    let mut following = Vec::new();
    for one in renamed {
        let paths = collect(
            connection,
            "SELECT path FROM local_entries \
             WHERE relocate_to = ?1 OR (relocate_to IS NULL AND path = ?1)",
            params![one.from().as_str()],
            OPERATION,
            |row| row.get::<_, String>(0).map_err(classify(OPERATION)),
        )?;
        following.extend(paths.into_iter().map(|path| (path, one.path.as_str())));
    }
    for (path, to) in following {
        connection
            .execute(
                "UPDATE local_entries SET relocate_to = NULLIF(?2, path) WHERE path = ?1",
                params![path, to],
            )
            .map_err(classify(OPERATION))?;
    }
    Ok(())
}

/// The files waiting to follow an Entry of one of `removed`, read before a
/// record takes those Containers out (spec: EP-10, FM-15).
pub(crate) fn following_any_of(
    connection: &Connection,
    removed: &[ContainerId],
) -> IndexResult<Vec<String>> {
    const OPERATION: &str = "reading the local files a removal leaves waiting";
    let mut paths = Vec::new();
    for container_id in removed {
        paths.extend(collect(
            connection,
            "SELECT local_entries.path FROM local_entries \
             JOIN entries ON entries.path = local_entries.relocate_to \
             WHERE entries.container_id = ?1",
            params![container_id.as_bytes().as_slice()],
            OPERATION,
            |row| row.get::<_, String>(0).map_err(classify(OPERATION)),
        )?);
    }
    Ok(paths)
}

/// Ends the wait of each file in `paths`, whose Entry a record's removals took
/// out, unless the path it waited for passed to a Container the record adds in
/// its place (spec: EP-6, EP-10, FM-15).
///
/// The file is then judged at the path it stands at rather than waiting for a
/// path that holds nothing it is a copy of (see [`end_wait`]).
pub(crate) fn release_vacated(
    connection: &Connection,
    paths: &[String],
    added: &[ContainerId],
) -> IndexResult<()> {
    const OPERATION: &str = "releasing the local files a removal left waiting";
    for path in paths {
        let standing = first(
            connection,
            "SELECT entries.container_id FROM local_entries \
             JOIN entries ON entries.path = local_entries.relocate_to \
             WHERE local_entries.path = ?1",
            params![path],
            OPERATION,
            |row| row.get::<_, Vec<u8>>(0).map_err(classify(OPERATION)),
        )?;
        let replaced = standing.is_some_and(|standing| {
            added
                .iter()
                .any(|container_id| container_id.as_bytes().as_slice() == standing.as_slice())
        });
        if !replaced {
            end_wait(connection, path, OPERATION)?;
        }
    }
    Ok(())
}

/// Ends the wait of the file at `path`, whose Entry has left the Library
/// (spec: EP-10, EP-15).
///
/// Where no current Entry stands at that path the row stays, following
/// nothing, and the file is departed there. Where one does, it is another Entry
/// than the one the file is a copy of, so the row is forgotten: a record kept
/// there would claim that Entry present and offer this file's bytes as its
/// edit. Forgotten, the file is one this device never placed for it (spec:
/// EP-11).
fn end_wait(connection: &Connection, path: &str, operation: &'static str) -> IndexResult<()> {
    connection
        .execute(
            "DELETE FROM local_entries WHERE path = ?1 \
             AND EXISTS (SELECT 1 FROM entries WHERE entries.path = ?1)",
            params![path],
        )
        .map_err(classify(operation))?;
    connection
        .execute(
            "UPDATE local_entries SET relocate_to = NULL WHERE path = ?1",
            params![path],
        )
        .map_err(classify(operation))?;
    Ok(())
}

/// Which Entry each local file is a copy of, by the name no rename changes:
/// the Entry at the path it waits to follow, or at its own (spec: FM-15).
pub(crate) fn followed_entries(
    connection: &Connection,
) -> IndexResult<Vec<(String, Vec<u8>, i64)>> {
    const OPERATION: &str = "reading which Entries the local files are copies of";
    collect(
        connection,
        "SELECT local_entries.path, entries.container_id, entries.entry_number \
         FROM local_entries \
         JOIN entries ON entries.path = COALESCE(local_entries.relocate_to, local_entries.path)",
        [],
        OPERATION,
        |row| {
            Ok((
                row.get::<_, String>(0).map_err(classify(OPERATION))?,
                row.get::<_, Vec<u8>>(1).map_err(classify(OPERATION))?,
                row.get::<_, i64>(2).map_err(classify(OPERATION))?,
            ))
        },
    )
}

/// Marks each file whose Entry an adopted Snapshot holds at another path to
/// follow it there, as the replay of the renames in between would have, and
/// ends the wait of every file waiting for a path that holds no Entry any more
/// (spec: CK-9, EP-10, FM-15; see [`end_wait`]).
///
/// An Entry the Snapshot no longer holds leaves its file following what stands
/// at the path it followed, if anything does, the way a replay leaves the file
/// of a replaced Entry (spec: EP-6).
pub(crate) fn follow_adopted(
    connection: &Connection,
    followed: &[(String, Vec<u8>, i64)],
) -> IndexResult<()> {
    const OPERATION: &str = "marking local files to follow an adopted Snapshot";
    for (path, container_id, entry_number) in followed {
        connection
            .execute(
                "UPDATE local_entries SET relocate_to = NULLIF(entries.path, local_entries.path) \
                 FROM entries \
                 WHERE local_entries.path = ?1 \
                 AND entries.container_id = ?2 AND entries.entry_number = ?3",
                params![path, container_id, entry_number],
            )
            .map_err(classify(OPERATION))?;
    }
    let stale = collect(
        connection,
        "SELECT path FROM local_entries \
         WHERE relocate_to IS NOT NULL \
         AND NOT EXISTS (SELECT 1 FROM entries WHERE entries.path = local_entries.relocate_to)",
        [],
        OPERATION,
        |row| row.get::<_, String>(0).map_err(classify(OPERATION)),
    )?;
    for path in &stale {
        end_wait(connection, path, OPERATION)?;
    }
    Ok(())
}
