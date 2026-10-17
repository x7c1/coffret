use coffret_usecase::{IndexResult, SnapshotContent};
use rusqlite::Connection;

use super::{insert_container, insert_entry, write_adopted_from, write_checkpoint};
use crate::device_state;
use crate::error::classify;

/// Replaces the whole Library-wide state with a Snapshot's content
/// (spec: CK-7, CK-9, RV-1).
pub(crate) fn restore(connection: &Connection, snapshot: SnapshotContent) -> IndexResult<()> {
    // Which Entry each local file is a copy of, read off the catalog being
    // replaced (spec: FM-15).
    let followed = device_state::followed_entries(connection)?;

    // Entries first: each one refers to a Container.
    connection
        .execute("DELETE FROM entries", [])
        .map_err(classify("clearing the Entries"))?;
    connection
        .execute("DELETE FROM containers", [])
        .map_err(classify("clearing the Containers"))?;

    let (checkpoint, adopted_from, containers, entries) = snapshot.into_parts();
    for container in &containers {
        insert_container(connection, container)?;
    }
    for entry in &entries {
        insert_entry(connection, entry)?;
    }
    device_state::follow_adopted(connection, &followed)?;
    write_checkpoint(connection, &checkpoint)?;
    write_adopted_from(connection, adopted_from.as_ref())
}
