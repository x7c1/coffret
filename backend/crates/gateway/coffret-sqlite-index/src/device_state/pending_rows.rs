use coffret_model::{ContainerId, ObjectRef};
use coffret_usecase::device_state::{AttemptedSlot, BatchId, PendingRow, SpoolState};
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::error::{classify, path_text};
use crate::query::collect;
use crate::rows;

/// Records a Container this device is about to spool, has spooled, or has
/// uploaded before its batch committed (spec: OC-2).
pub(crate) fn record_pending_row(connection: &Connection, pending: &PendingRow) -> IndexResult<()> {
    const OPERATION: &str = "recording a spool";
    let spool_path = path_text(&pending.spool_path, OPERATION)?;

    connection
        .execute(
            "INSERT INTO pending_rows \
                 (container_id, spool_path, state, batch, created_at, object_ref, commit_attempted,
                  materializes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT (container_id) DO UPDATE SET
                 spool_path = excluded.spool_path,
                 state = excluded.state,
                 batch = excluded.batch,
                 created_at = excluded.created_at,
                 object_ref = excluded.object_ref,
                 commit_attempted = excluded.commit_attempted,
                 materializes = excluded.materializes",
            params![
                pending.container_id.as_bytes().as_slice(),
                spool_path,
                rows::spool_state_text(&pending.state),
                pending.batch.as_str(),
                pending.created_at.as_unix_seconds(),
                pending.state.object_ref().map(ObjectRef::as_str),
                pending.commit_attempted,
                pending.materializes,
            ],
        )
        .map_err(classify(OPERATION))?;
    Ok(())
}

/// Records that one Container's spool file is whole (spec: OC-2).
///
/// An `UPDATE` and not an upsert, for the reason [`mark_absent`](super::mark_absent) is one: a
/// Container with no row is one no spool step of this device ever announced, and
/// giving it one would record a spool that does not exist and set a later run
/// looking for a file nobody wrote. Everything the announcing row said about the
/// Container stays — the path, the batch, the moment it was announced — because
/// none of it changed when the file did.
pub(crate) fn mark_spooled(connection: &Connection, container_id: ContainerId) -> IndexResult<()> {
    connection
        .execute(
            "UPDATE pending_rows SET state = ?2 WHERE container_id = ?1",
            params![
                container_id.as_bytes().as_slice(),
                rows::spool_state_text(&SpoolState::Spooled(None)),
            ],
        )
        .map_err(classify("marking a Container spooled"))?;
    Ok(())
}

/// Marks the pending rows of a batch whose Journal record is about to be sent
/// into `slot`, and records the slot against the batch of every row it marks
/// (spec: OC-2, OC-3).
///
/// One `UPDATE` and one `INSERT` per Container, all inside the caller's
/// transaction: a failure on any of them rolls back the ones before it, so the
/// batch is marked whole with its slot recorded, or not at all. Like
/// [`mark_spooled`], an update and not an upsert: the `INSERT` selects the
/// batch from the row it has just marked, so a Container with no row records
/// nothing either. A slot already recorded for the batch is left as it is.
pub(crate) fn mark_commit_attempted(
    connection: &Connection,
    container_ids: &[ContainerId],
    slot: &AttemptedSlot,
) -> IndexResult<()> {
    const OPERATION: &str = "marking a commit attempted";
    let generation = rows::to_integer(OPERATION, "generation", slot.generation().get())?;
    let mut mark = connection
        .prepare_cached("UPDATE pending_rows SET commit_attempted = 1 WHERE container_id = ?1")
        .map_err(classify(OPERATION))?;
    let mut record = connection
        .prepare_cached(
            "INSERT OR IGNORE INTO attempted_slots (batch, generation, provider_id)
             SELECT batch, ?2, ?3 FROM pending_rows WHERE container_id = ?1",
        )
        .map_err(classify(OPERATION))?;
    for container_id in container_ids {
        let container_id = container_id.as_bytes().as_slice();
        mark.execute(params![container_id])
            .map_err(classify(OPERATION))?;
        record
            .execute(params![container_id, generation, slot.provider_id()])
            .map_err(classify(OPERATION))?;
    }
    Ok(())
}

/// Every commit slot recorded against one batch, by generation and then by
/// identifier (spec: OC-2, OC-3).
pub(crate) fn attempted_slots(
    connection: &Connection,
    batch: &BatchId,
) -> IndexResult<Vec<AttemptedSlot>> {
    collect(
        connection,
        "SELECT * FROM attempted_slots WHERE batch = ?1 \
         ORDER BY generation, ifnull(provider_id, '')",
        params![batch.as_str()],
        "reading the attempted commit slots",
        rows::attempted_slot,
    )
}

/// Drops the pending row for one Container, its batch having committed or been
/// abandoned, and the slots its batch recorded once no row of the batch is
/// left.
///
/// Dropping one that is not there is a no-op, so an interrupted cleanup is
/// simply run again (spec: OC-8).
pub(crate) fn clear_pending_row(
    connection: &Connection,
    container_id: ContainerId,
) -> IndexResult<()> {
    const OPERATION: &str = "clearing a spool";
    connection
        .execute(
            "DELETE FROM pending_rows WHERE container_id = ?1",
            params![container_id.as_bytes().as_slice()],
        )
        .map_err(classify(OPERATION))?;
    connection
        .execute(
            "DELETE FROM attempted_slots \
             WHERE batch NOT IN (SELECT batch FROM pending_rows)",
            [],
        )
        .map_err(classify(OPERATION))?;
    Ok(())
}

/// Every Container this device is about to spool, has spooled, or has uploaded
/// whose batch has not committed (spec: OC-2).
///
/// `state` comes back with each row, which is what tells a spool this device only
/// announced from one it finished writing.
pub(crate) fn pending_rows(connection: &Connection) -> IndexResult<Vec<PendingRow>> {
    collect(
        connection,
        "SELECT * FROM pending_rows ORDER BY container_id",
        [],
        "reading the spools",
        rows::pending_row,
    )
}
