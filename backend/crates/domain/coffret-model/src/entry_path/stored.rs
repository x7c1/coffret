use unicode_normalization::is_nfc;

use super::{defect_in, EntryPath};
use crate::error::{Error, Result};

impl EntryPath {
    /// The Entry Path a stored path already is, or a refusal where it is not
    /// (spec: EP-1, EP-2).
    ///
    /// Nothing is normalized and nothing is corrected: a stored path is asked
    /// whether it is already what every Entry Path is, and a path that is not is
    /// malformed data. What a reader makes of that refusal is its own layer's
    /// business — a Container that does not open, a control object that does not
    /// decode, a catalog that is rebuilt from Storage — and the one answer no
    /// layer gives is to fix it up and carry on.
    ///
    /// # Errors
    ///
    /// [`Error::UnnormalizedEntryPath`] where `text` is not NFC, and
    /// [`Error::MalformedEntryPath`] where it is NFC and outside the shape.
    /// The normal form is asked about first, because a path that is neither is a
    /// record written by something that held to neither rule and either answer
    /// is the same verdict.
    pub fn stored(text: impl Into<String>) -> Result<Self> {
        let text = text.into();
        if !is_nfc(&text) {
            return Err(Error::UnnormalizedEntryPath { path: text });
        }
        match defect_in(&text) {
            Some(defect) => Err(Error::MalformedEntryPath { path: text, defect }),
            None => Ok(Self(text)),
        }
    }
}
