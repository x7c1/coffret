use coffret_model::{ContainerId, ContainerKind, ContentHash, EntryPath};
use coffret_usecase::device_state::{LocalEntryState, SpoolState};
use coffret_usecase::{IndexError, IndexResult};
use rusqlite::Row;

use crate::error::{negative, translate, unreadable_model};

/// How an unsigned domain value is spelled in an INTEGER column, or a refusal
/// where it has no spelling there.
///
/// SQLite integers are 64 bits and signed, while offsets, sizes, generations,
/// and epochs are unsigned, so the top half of the unsigned range is the one
/// part of it a column cannot hold. Refusing a value that reaches it — which
/// nothing the format produces does, as [`IndexError::UnrepresentableValue`]
/// sets out — keeps every integer in the file a number a writer could have put
/// there, which is what makes a negative one on the way back a sign of a
/// damaged or hand-edited file rather than an ordinary large value.
///
/// # Errors
///
/// [`IndexError::UnrepresentableValue`] where `value` is `2^63` or above.
pub(crate) fn to_integer(
    operation: &'static str,
    column: &'static str,
    value: u64,
) -> IndexResult<i64> {
    i64::try_from(value).map_err(|_| IndexError::UnrepresentableValue {
        operation,
        column,
        value,
    })
}

/// The inverse of [`to_integer`], reading one column of a row.
///
/// # Errors
///
/// [`IndexError::UnreadableCatalog`] where the column holds a negative
/// integer, and whatever [`integer`] reports where it holds no integer at all.
pub(super) fn from_integer(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<u64> {
    let value = integer(row, column, operation)?;
    u64::try_from(value).map_err(|_| negative(operation, column, value))
}

/// How a Container's kind is spelled, in the meta section's own vocabulary
/// (spec: FM-9, PK-15).
pub(crate) const fn kind_text(kind: ContainerKind) -> &'static str {
    match kind {
        ContainerKind::OneFile => "one-file",
        ContainerKind::Pack => "pack",
    }
}

/// How this device's answer to "do I have this file" is spelled (spec: EP-10).
pub(crate) const fn state_text(state: LocalEntryState) -> &'static str {
    match state {
        LocalEntryState::Present => "present",
        LocalEntryState::Absent => "absent",
    }
}

/// How this device's answer to "is that spool file a whole Container" is spelled
/// (spec: OC-2).
pub(crate) const fn spool_state_text(state: SpoolState) -> &'static str {
    match state {
        SpoolState::Spooling => "spooling",
        SpoolState::Spooled => "spooled",
    }
}

pub(super) fn integer(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<i64> {
    row.get(column).map_err(translate(operation))
}

pub(super) fn optional_integer(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<Option<i64>> {
    row.get(column).map_err(translate(operation))
}

pub(super) fn text(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<String> {
    row.get(column).map_err(translate(operation))
}

pub(super) fn optional_text(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<Option<String>> {
    row.get(column).map_err(translate(operation))
}

pub(super) fn optional_blob(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<Option<Vec<u8>>> {
    row.get(column).map_err(translate(operation))
}

/// An Entry Path the catalog holds, which is already the NFC spelling every
/// Entry Path is in (spec: EP-1).
///
/// A row that is not is a row this build cannot read: the file is a cache that
/// can be rebuilt from Storage (spec: RV-5), so saying so and letting it be
/// discarded is the honest answer, where composing the path would quietly hand
/// callers a Library position no committed state ever held.
pub(super) fn entry_path(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<EntryPath> {
    EntryPath::stored(text(row, column, operation)?).map_err(unreadable_model(operation))
}

/// The same, for a column that may hold no path at all.
pub(super) fn optional_entry_path(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<Option<EntryPath>> {
    optional_text(row, column, operation)?
        .map(EntryPath::stored)
        .transpose()
        .map_err(unreadable_model(operation))
}

pub(super) fn container_id(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<ContainerId> {
    let bytes: Vec<u8> = row.get(column).map_err(translate(operation))?;
    ContainerId::from_slice(&bytes).map_err(unreadable_model(operation))
}

pub(super) fn content_hash(
    row: &Row<'_>,
    column: &'static str,
    operation: &'static str,
) -> IndexResult<ContentHash> {
    let bytes: Vec<u8> = row.get(column).map_err(translate(operation))?;
    ContentHash::from_slice(&bytes).map_err(unreadable_model(operation))
}
