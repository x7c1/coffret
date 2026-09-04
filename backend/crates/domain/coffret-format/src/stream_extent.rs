use coffret_model::{EntryExtent, Error as ModelError};

use crate::error::{Error, Result};

/// The extent of `size` bytes at `offset`, in this crate's vocabulary (FM-9).
///
/// The rule and its reason are [`EntryExtent::new`]'s: an Entry's `offset` and
/// `size` describe a range of a plaintext stream addressed in 64 bits, so their
/// sum stays inside it. This restates that refusal as [`Error::StreamTooLong`]
/// — the variant the entry table's own walk already raises for the same
/// overflow — so one Container yields one error whichever check catches it,
/// whether the table came out of a meta section, out of a control payload, or
/// out of the layout a writer was drawing.
pub(crate) fn stream_extent(offset: u64, size: u64) -> Result<EntryExtent> {
    EntryExtent::new(offset, size).map_err(refusal)
}

/// The extent of the Entry laid directly after `previous`, on the same terms
/// (spec: FM-4, FM-9).
pub(crate) fn extent_after(previous: EntryExtent, size: u64) -> Result<EntryExtent> {
    previous.following(size).map_err(refusal)
}

/// The model's refusal as this crate states it.
///
/// The offending pair is dropped rather than kept as a cause, which is what
/// makes this a `map_err` rather than a `?`: a second spelling of
/// [`Error::StreamTooLong`] would only be a second thing for a caller to match
/// on. Anything else the model refuses for would be a rule this layer has not
/// been told about, and passing it through says so.
fn refusal(error: ModelError) -> Error {
    match error {
        ModelError::ExtentPastTheAddressSpace { .. } => Error::StreamTooLong,
        other => Error::Model(other),
    }
}
