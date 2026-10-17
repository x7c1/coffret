//! Keeping each materialization record about the Entry it names, as the
//! Library-wide operations move the catalog on (spec: EP-6, EP-10, FM-15).

use std::collections::BTreeSet;

use coffret_model::ContainerId;
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::error::classify;
use crate::query::{collect, first};

/// A row's own path and the path the Entry it is a copy of stands at, read
/// before the catalog changes.
pub(crate) type Stood = (String, String);

/// Where the Entry of each row whose Entry one of `removed` holds stands, read
/// before a record takes those Containers out: a replacement takes the row over
/// at that path (spec: EP-6).
pub(crate) fn standing_in(
    connection: &Connection,
    removed: &[ContainerId],
) -> IndexResult<Vec<Stood>> {
    const OPERATION: &str = "reading the local files a removal reaches";
    let mut stood = Vec::new();
    for container_id in removed {
        stood.extend(collect(
            connection,
            "SELECT local_entries.path, entries.path FROM local_entries \
             JOIN entries ON entries.container_id = local_entries.entry_container \
             AND entries.entry_number = local_entries.entry_number \
             WHERE local_entries.entry_container = ?1",
            params![container_id.as_bytes().as_slice()],
            OPERATION,
            stood_row(OPERATION),
        )?);
    }
    Ok(stood)
}

/// Where the Entry of every row whose Entry is current stands, read before a
/// restore replaces the catalog (spec: CK-9).
pub(crate) fn standing(connection: &Connection) -> IndexResult<Vec<Stood>> {
    const OPERATION: &str = "reading which Entries the local files are copies of";
    collect(
        connection,
        "SELECT local_entries.path, entries.path FROM local_entries \
         JOIN entries ON entries.container_id = local_entries.entry_container \
         AND entries.entry_number = local_entries.entry_number",
        [],
        OPERATION,
        stood_row(OPERATION),
    )
}

/// The paths of the rows whose Entry is the one numbered `entry_number` in
/// `container_id`.
pub(crate) fn copies_of(
    connection: &Connection,
    container_id: ContainerId,
    entry_number: u32,
) -> IndexResult<Vec<String>> {
    const OPERATION: &str = "reading the local files of a renamed Entry";
    collect(
        connection,
        "SELECT path FROM local_entries WHERE entry_container = ?1 AND entry_number = ?2",
        params![container_id.as_bytes().as_slice(), entry_number],
        OPERATION,
        |row| row.get::<_, String>(0).map_err(classify(OPERATION)),
    )
}

/// Carries each row in `stood` over to the Entry now standing where its own
/// stood, where `replaces` says that Entry's Container takes its place
/// (spec: EP-6, EP-10).
pub(crate) fn carry_over(
    connection: &Connection,
    stood: &[Stood],
    replaces: impl Fn(&[u8]) -> bool,
) -> IndexResult<()> {
    const OPERATION: &str = "carrying a local file's row over to a replacement";
    for (path, stood_at) in stood {
        let successor = first(
            connection,
            "SELECT container_id, entry_number FROM entries WHERE path = ?1",
            params![stood_at],
            OPERATION,
            |row| {
                Ok((
                    row.get::<_, Vec<u8>>(0).map_err(classify(OPERATION))?,
                    row.get::<_, i64>(1).map_err(classify(OPERATION))?,
                ))
            },
        )?;
        let Some((container_id, entry_number)) = successor.filter(|(id, _)| replaces(id)) else {
            continue;
        };
        connection
            .execute(
                "UPDATE local_entries SET entry_container = ?2, entry_number = ?3 WHERE path = ?1",
                params![path, container_id, entry_number],
            )
            .map_err(classify(OPERATION))?;
    }
    Ok(())
}

/// The IDs of every Container the catalog holds now.
pub(crate) fn current_containers(connection: &Connection) -> IndexResult<BTreeSet<Vec<u8>>> {
    const OPERATION: &str = "reading which Containers are current";
    Ok(collect(
        connection,
        "SELECT id FROM containers",
        [],
        OPERATION,
        |row| row.get::<_, Vec<u8>>(0).map_err(classify(OPERATION)),
    )?
    .into_iter()
    .collect())
}

/// Whether the catalog holds the Entry the row at `path` is a copy of.
pub(crate) fn holds_entry_of(connection: &Connection, path: &str) -> IndexResult<bool> {
    connection
        .query_row(
            "SELECT EXISTS (
                 SELECT 1 FROM local_entries JOIN entries
                 ON entries.container_id = local_entries.entry_container
                 AND entries.entry_number = local_entries.entry_number
                 WHERE local_entries.path = ?1
             )",
            params![path],
            |row| row.get::<_, bool>(0),
        )
        .map_err(classify("reading whether a local file's Entry is current"))
}

fn stood_row(operation: &'static str) -> impl Fn(&rusqlite::Row<'_>) -> IndexResult<Stood> {
    move |row| {
        Ok((
            row.get::<_, String>(0).map_err(classify(operation))?,
            row.get::<_, String>(1).map_err(classify(operation))?,
        ))
    }
}
