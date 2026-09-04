use coffret_model::{EntryPath, Error as ModelError};

use crate::error::{Error, Result};

/// One Entry Path out of a decoded entry map, whichever spells it (FM-9,
/// FM-15).
///
/// The rules and their reasons are [`EntryPath::stored`]'s; this restates its
/// two refusals in the format layer's vocabulary, where a decoded path that is
/// not NFC ([`Error::UnnormalizedEntryPath`], EP-1) and one that is not in the
/// shape every Entry Path is in ([`Error::MalformedEntryPath`], EP-2) are each
/// one more malformed payload.
///
/// The model's refusal is dropped rather than kept as a cause, which is what
/// the `map_err` is here for: `?` on its own would take the [`Error::Model`]
/// conversion, and what either refusal carries is the offending path, which no
/// error this crate raises carries — so naming the field is the whole of what
/// may be said about it. The shape refusal names which part of the shape went
/// as well, and that goes with the path because it settles nothing here: an
/// object carrying a path no writer holding to EP-2 could have written does not
/// decode, whichever part of the shape it was that went.
pub(crate) fn stored_path(text: &str, field: &'static str) -> Result<EntryPath> {
    EntryPath::stored(text).map_err(|error| match error {
        ModelError::UnnormalizedEntryPath { .. } => Error::UnnormalizedEntryPath { field },
        ModelError::MalformedEntryPath { .. } => Error::MalformedEntryPath { field },
        // Those two are what `stored` refuses for. Anything else would be a
        // rule this layer has not been told about, and passing it through says
        // so rather than filing it under one of the two that were.
        other => Error::Model(other),
    })
}
