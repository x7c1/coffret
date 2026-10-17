use coffret_model::EntryPath;
use coffret_usecase::device_state::{Mapping, RootIdentity, RootMarkerId};
use coffret_usecase::IndexResult;
use rusqlite::{params, Connection};

use crate::error::{classify, path_text};
use crate::query::collect;
use crate::rows;

/// Records where one part of the Library lives on this device (spec: EP-9).
///
/// `IS` rather than `=` because the Library root is stored as NULL, and NULL is
/// equal to nothing, itself included.
///
/// The `DELETE` and `INSERT` replace the whole row, which is what makes the
/// re-confirmation gesture work with no extra statement: a mapping recorded with
/// no [`root_identity`](Mapping::root_identity) clears the column, so the next
/// scan stamps whatever is there rather than comparing against what an earlier
/// run recorded (spec: EP-12). The identity the mapping expects of its root
/// travels the same way and for the same reason — the row is whatever the caller
/// recorded, and what a registration wrote or adopted is what it says
/// (spec: EP-13).
pub(crate) fn set_mapping(connection: &Connection, mapping: &Mapping) -> IndexResult<()> {
    const OPERATION: &str = "recording a mapping";
    let prefix = mapping.prefix.as_ref().map(EntryPath::as_str);
    let local_root = path_text(&mapping.local_root, OPERATION)?;
    let root_identity = mapping.root_identity.as_ref().map(RootIdentity::as_str);
    let expected_root_id = mapping.expected_root_id.as_ref().map(RootMarkerId::to_hex);

    connection
        .execute("DELETE FROM mappings WHERE prefix IS ?1", params![prefix])
        .map_err(classify(OPERATION))?;
    connection
        .execute(
            "INSERT INTO mappings (prefix, local_root, root_identity, expected_root_id) \
             VALUES (?1, ?2, ?3, ?4)",
            params![prefix, local_root, root_identity, expected_root_id],
        )
        .map_err(classify(OPERATION))?;
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
