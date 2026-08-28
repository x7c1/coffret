use coffret_model::EntryPath;

use crate::error::{Error, Result};

/// One Entry Path out of a decoded entry table (FM-9).
///
/// The rule and its reason are [`EntryPath::stored`]'s; this restates the
/// refusal in the format layer's vocabulary, where a decoded path that is not
/// NFC is one more malformed payload ([`Error::UnnormalizedEntryPath`]).
///
/// The model's refusal is dropped rather than kept as a cause, which is what
/// the `map_err` is here for: `?` on its own would take the [`Error::Model`]
/// conversion, and all that refusal carries is the offending path. No error
/// this crate raises carries a payload value, so naming the field is the whole
/// of what may be reported and nothing is lost that could have been said.
pub(super) fn stored_path(text: &str, field: &'static str) -> Result<EntryPath> {
    EntryPath::stored(text).map_err(|_| Error::UnnormalizedEntryPath { field })
}
