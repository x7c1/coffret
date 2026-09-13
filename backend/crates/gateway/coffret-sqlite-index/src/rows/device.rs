use std::path::PathBuf;

use coffret_model::{Mtime, ObjectRef};
use coffret_usecase::device_state::{
    BatchId, DeviceTime, LocalEntry, LocalEntryState, LocalObservation, Mapping, PendingUpload,
    RootIdentity, RootMarkerId, SpoolState,
};
use coffret_usecase::{IndexError, IndexResult};
use rusqlite::Row;

use super::columns::{
    container_id, entry_path, from_integer, integer, optional_entry_path, optional_text, text,
};
use crate::error::unreadable;

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
/// what stamps the one and the recording itself is what settles the other —
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

/// One row of `pending_uploads`.
pub(crate) fn pending_upload(row: &Row<'_>) -> IndexResult<PendingUpload> {
    const OPERATION: &str = "reading a spool";
    Ok(PendingUpload {
        container_id: container_id(row, "container_id", OPERATION)?,
        spool_path: PathBuf::from(text(row, "spool_path", OPERATION)?),
        batch: BatchId::new(text(row, "batch", OPERATION)?),
        created_at: DeviceTime::from_unix_seconds(integer(row, "created_at", OPERATION)?),
        state: match text(row, "state", OPERATION)?.as_str() {
            "spooling" => SpoolState::Spooling,
            "spooled" => SpoolState::Spooled,
            found => return Err(unreadable(OPERATION, "spool state", found)),
        },
        object_ref: optional_text(row, "object_ref", OPERATION)?.map(ObjectRef::new),
    })
}
