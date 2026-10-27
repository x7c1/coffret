use coffret_usecase::{IndexResult, SnapshotContent};
use rusqlite::Connection;

use super::{
    insert_container, insert_entry, write_adopted_from, write_checkpoint, write_library_name,
};
use crate::device_state;
use crate::error::classify;

/// Replaces the whole Library-wide state with a Snapshot's content
/// (spec: CK-7, CK-9, RV-1).
pub(crate) fn restore(connection: &Connection, snapshot: SnapshotContent) -> IndexResult<()> {
    // Where each record's Entry stands in the catalog being replaced, and which
    // Containers that catalog holds: what tells a replacement from an Entry
    // that was already there (spec: EP-6).
    let stood = device_state::standing(connection)?;
    let replaced = device_state::current_containers(connection)?;

    // Entries first: each one refers to a Container.
    connection
        .execute("DELETE FROM entries", [])
        .map_err(classify("clearing the Entries"))?;
    connection
        .execute("DELETE FROM containers", [])
        .map_err(classify("clearing the Containers"))?;

    let name = snapshot.library_name().cloned();
    let (checkpoint, adopted_from, containers, entries) = snapshot.into_parts();
    for container in &containers {
        insert_container(connection, container)?;
    }
    for entry in &entries {
        insert_entry(connection, entry)?;
    }
    // A record whose Entry the Snapshot still holds stays with it, wherever it
    // stands now. One whose Entry is gone is carried over to the Entry standing
    // where its own stood, but only where that Entry's Container is new: a
    // replacement always brings one, and an Entry that was current already —
    // one a rename brought there — is another Entry than the one the file is a
    // copy of (spec: EP-6, FM-15).
    let mut gone = Vec::with_capacity(stood.len());
    for one in stood {
        if !device_state::holds_entry_of(connection, &one.0)? {
            gone.push(one);
        }
    }
    device_state::carry_over(connection, &gone, |container_id| {
        !replaced.contains(container_id)
    })?;
    device_state::reconcile_all(
        connection,
        "keeping the local files' rows about their Entries",
    )?;
    write_checkpoint(connection, &checkpoint)?;
    // The Snapshot's name replaces the catalog's, whichever it is, a Snapshot
    // naming nothing included: a restore replaces the whole Library-wide state
    // (spec: FM-16, RV-1).
    write_library_name(connection, name.as_ref())?;
    write_adopted_from(connection, adopted_from.as_ref())
}
