use std::path::PathBuf;

use coffret_model::{Mtime, ObjectRef};
use coffret_usecase::device_state::{
    BatchId, DeviceTime, LocalEntry, LocalEntryState, LocalObservation, Mapping, PendingUpload,
    RootIdentity, SpoolState,
};
use coffret_usecase::IndexResult;
use rusqlite::Row;

use super::columns::{
    container_id, entry_path, from_integer, integer, optional_entry_path, optional_text, text,
};
use crate::error::unreadable;

/// One row of `mappings`.
pub(crate) fn mapping(row: &Row<'_>) -> IndexResult<Mapping> {
    const OPERATION: &str = "reading a mapping";
    Ok(Mapping {
        prefix: optional_entry_path(row, "prefix", OPERATION)?,
        local_root: PathBuf::from(text(row, "local_root", OPERATION)?),
        root_identity: optional_text(row, "root_identity", OPERATION)?.map(RootIdentity::new),
    })
}

/// The two columns of `mappings` a refused file still keeps readable by name
/// (the two columns every layout keeps, next to `DEVICE_SCHEMA_VERSION`).
///
/// `root_identity` always comes back `None`: a mapping read out of a refused
/// file is about to be recorded afresh, so the next scan is what stamps it,
/// the same as `set_mapping` treats a mapping recorded for the first time.
pub(crate) fn refused_mapping(row: &Row<'_>) -> IndexResult<Mapping> {
    const OPERATION: &str = "reading a mapping from a refused Index file";
    Ok(Mapping {
        prefix: optional_entry_path(row, "prefix", OPERATION)?,
        local_root: PathBuf::from(text(row, "local_root", OPERATION)?),
        root_identity: None,
    })
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
