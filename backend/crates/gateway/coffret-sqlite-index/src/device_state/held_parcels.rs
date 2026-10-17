use coffret_model::ContainerId;
use coffret_usecase::device_state::HeldParcel;
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::error::{classify, path_text};
use crate::query::collect;
use crate::rows;

/// Records that this device holds one parcel, replacing any row already held
/// for it (spec: PK-21).
pub(crate) fn hold_parcel(connection: &Connection, parcel: &HeldParcel) -> IndexResult<()> {
    const OPERATION: &str = "recording a held parcel";
    let path = path_text(&parcel.path, OPERATION)?;
    connection
        .execute(
            "INSERT INTO held_parcels (container_id, parcel, plaintext_start, plaintext_end, file_path)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (container_id, parcel) DO UPDATE SET
                 plaintext_start = excluded.plaintext_start,
                 plaintext_end = excluded.plaintext_end,
                 file_path = excluded.file_path",
            params![
                parcel.container_id.as_bytes().as_slice(),
                rows::to_integer(OPERATION, "parcel", parcel.index)?,
                rows::to_integer(OPERATION, "plaintext_start", parcel.plaintext.start)?,
                rows::to_integer(OPERATION, "plaintext_end", parcel.plaintext.end)?,
                path,
            ],
        )
        .map_err(classify(OPERATION))?;
    Ok(())
}

/// Every parcel this device holds, by Container and then by parcel index
/// (spec: PK-21).
pub(crate) fn held_parcels(connection: &Connection) -> IndexResult<Vec<HeldParcel>> {
    collect(
        connection,
        "SELECT * FROM held_parcels ORDER BY container_id, parcel",
        [],
        "reading the held parcels",
        rows::held_parcel,
    )
}

/// Forgets one held parcel's row. One that is not there is a no-op, so an
/// interrupted letting go is simply run again (spec: OC-8, PK-21).
pub(crate) fn let_go_parcel(
    connection: &Connection,
    container_id: ContainerId,
    index: u64,
) -> IndexResult<()> {
    const OPERATION: &str = "letting go of a held parcel";
    connection
        .execute(
            "DELETE FROM held_parcels WHERE container_id = ?1 AND parcel = ?2",
            params![
                container_id.as_bytes().as_slice(),
                rows::to_integer(OPERATION, "parcel", index)?
            ],
        )
        .map_err(classify(OPERATION))?;
    Ok(())
}
