use std::path::PathBuf;

use coffret_model::{Mtime, ObjectRef};
use coffret_usecase::device_state::{
    BatchId, DeviceTime, LocalEntry, LocalEntryState, LocalObservation, Mapping, PendingRow,
    RootIdentity, RootMarkerId, SpoolState,
};
use coffret_usecase::{IndexError, IndexResult};
use rusqlite::Row;

use super::columns::{
    container_id, entry_path, from_integer, integer, optional_entry_path, optional_text, text,
};
use crate::error::{classify, object_on_spooling_row, unreadable};

/// One row of `mappings`.
pub(crate) fn mapping(row: &Row<'_>) -> IndexResult<Mapping> {
    const OPERATION: &str = "reading a mapping";
    let mut mapping = Mapping::new(
        optional_entry_path(row, "prefix", OPERATION)?,
        PathBuf::from(text(row, "local_root", OPERATION)?),
    );
    if let Some(identity) = optional_text(row, "root_identity", OPERATION)? {
        mapping = mapping.stamped(RootIdentity::new(identity));
    }
    // A column holding anything but the sixteen characters an identity is
    // spelled in gets the verdict a prefix that is not NFC gets: a catalog this
    // build cannot read. Reading it as no identity at all would turn a mapping
    // whose marker is checked into one nothing may be placed through, and
    // silently (spec: EP-13).
    if let Some(spelling) = optional_text(row, "expected_root_id", OPERATION)? {
        // The domain's own refusal is what travels, the way it does for every
        // other stored value a model type reads back (see `unreadable_model`):
        // it names whether the spelling was the wrong length or held a
        // character no identity is spelled with, which the row reader would
        // only be guessing at. `unreadable` is for a column no type refuses —
        // a state word, a count — and flattening a typed refusal into one of
        // those would throw that away.
        let expected =
            RootMarkerId::parse(&spelling).map_err(|cause| IndexError::UnreadableCatalog {
                operation: OPERATION,
                cause: Box::new(cause),
            })?;
        mapping = mapping.expecting(expected);
    }
    Ok(mapping)
}

/// The two columns of `mappings` a refused file still keeps readable by name
/// (the two columns every layout keeps, next to `DEVICE_SCHEMA_VERSION`).
///
/// `root_identity` and `expected_root_id` both come back `None`: a mapping read
/// out of a refused file is about to be recorded afresh, so the next scan is
/// what stamps the one and the recording itself is what decides the other —
/// the same as `set_mapping` treats a mapping recorded for the first time
/// (spec: EP-12, EP-13).
pub(crate) fn refused_mapping(row: &Row<'_>) -> IndexResult<Mapping> {
    const OPERATION: &str = "reading a mapping from a refused Index file";
    Ok(Mapping::new(
        optional_entry_path(row, "prefix", OPERATION)?,
        PathBuf::from(text(row, "local_root", OPERATION)?),
    ))
}

/// One row of `local_entries`.
pub(crate) fn local_entry(row: &Row<'_>) -> IndexResult<LocalEntry> {
    const OPERATION: &str = "reading a local file's row";
    Ok(LocalEntry {
        observation: LocalObservation {
            path: entry_path(row, "path", OPERATION)?,
            size: from_integer(row, "observed_size", OPERATION)?,
            mtime: Mtime::from_unix_seconds(integer(row, "observed_mtime", OPERATION)?),
            at: DeviceTime::from_unix_seconds(integer(row, "observed_at", OPERATION)?),
        },
        state: match text(row, "state", OPERATION)?.as_str() {
            "present" => LocalEntryState::Present,
            "absent" => LocalEntryState::Absent,
            found => return Err(unreadable(OPERATION, "local file state", found)),
        },
    })
}

/// One row of `pending_rows`.
pub(crate) fn pending_row(row: &Row<'_>) -> IndexResult<PendingRow> {
    const OPERATION: &str = "reading a spool";
    Ok(PendingRow {
        commit_attempted: row.get("commit_attempted").map_err(classify(OPERATION))?,
        container_id: container_id(row, "container_id", OPERATION)?,
        spool_path: PathBuf::from(text(row, "spool_path", OPERATION)?),
        batch: BatchId::new(text(row, "batch", OPERATION)?),
        created_at: DeviceTime::from_unix_seconds(integer(row, "created_at", OPERATION)?),
        state: spool_state(row, OPERATION)?,
    })
}

/// The state column and the object handle beside it, read as the one value
/// they are.
///
/// The stored form keeps them apart, so a file can hold a `spooling` row that
/// names an object; no writer of this catalog produces one, and it is refused
/// the way a state this build does not know is.
fn spool_state(row: &Row<'_>, operation: &'static str) -> IndexResult<SpoolState> {
    let object_ref = optional_text(row, "object_ref", operation)?.map(ObjectRef::new);
    match (text(row, "state", operation)?.as_str(), object_ref) {
        ("spooling", None) => Ok(SpoolState::Spooling),
        ("spooling", Some(_)) => Err(object_on_spooling_row(operation)),
        ("spooled", object_ref) => Ok(SpoolState::Spooled(object_ref)),
        (found, _) => Err(unreadable(operation, "spool state", found)),
    }
}
