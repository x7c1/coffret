//! One element of a Journal record's `renames` (spec: FM-15).
//!
//! Three fields: `container`, the 16-byte Container ID; `entry`, the renamed
//! Entry's entry number in that Container's table (spec: FM-9); and `path`, the
//! Entry Path it stands at from this record on. The name it leaves is not
//! carried, because it is the catalog's.

use ciborium::Value;
use coffret_model::{ContainerId, EntryRename};

use crate::control::cbor::{entry_number, Fields, MapBuilder};
use crate::error::{Error, Result};
use crate::meta::stored_path;

const CONTAINER: &str = "container";
const ENTRY: &str = "entry";
const PATH: &str = "path";

/// The element's map, keyed by name until the body is written.
pub(super) fn to_value(rename: &EntryRename) -> Value {
    MapBuilder::new()
        .bytes(CONTAINER, rename.container_id().as_bytes())
        .uint(ENTRY, u64::from(rename.entry_number().get()))
        .text(PATH, rename.path().as_str())
        .build()
}

/// The element at `index` of `renames`, or the refusal naming it.
///
/// The path is a stored Entry Path, held to EP-1 and EP-2 as every other path a
/// record carries is, and then to the reserved derived namespace, which is the
/// one rule a rename's path has beyond those (spec: FM-15).
pub(super) fn from_fields(
    index: usize,
    fields: &Fields<'_>,
    malformed: fn(String) -> Error,
) -> Result<EntryRename> {
    let container_id =
        ContainerId::from_bytes(fields.byte_array::<{ ContainerId::BYTE_LEN }>(CONTAINER)?);
    let entry = entry_number(fields, ENTRY, malformed)?;
    let path = stored_path(&fields.text(PATH)?, PATH)?;
    EntryRename::new(container_id, entry, path).map_err(|error| match error {
        coffret_model::Error::RenameIntoReservedNamespace => {
            Error::RenameIntoReservedNamespace { rename: index }
        }
        other => Error::Model(other),
    })
}
