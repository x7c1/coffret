use std::str::FromStr;

use unicode_normalization::{is_nfc, UnicodeNormalization};

use super::{defect_in, EntryPath};
use crate::error::{Error, Result};

impl EntryPath {
    /// The Entry Path a piece of text from outside the Library stands for, or a
    /// refusal where it stands for none (spec: EP-1, EP-2).
    ///
    /// Composed first and checked second. The order decides nothing about the
    /// verdict — composition neither creates nor removes a `/`, a NUL, or an
    /// empty component — so the shape a refusal reports is the shape of the text
    /// as it was typed, which is why the refusal quotes that rather than the
    /// composed spelling nobody typed.
    ///
    /// Idempotent on the composing side: text already in NFC is kept as it
    /// stands, which is the ordinary case and the reason nothing is copied for
    /// it.
    ///
    /// # Errors
    ///
    /// [`Error::MalformedEntryPath`] where the text is not in the shape EP-2
    /// spells, carrying the part of the shape it failed.
    pub fn parse(text: impl Into<String>) -> Result<Self> {
        let text = text.into();
        let composed = (!is_nfc(&text)).then(|| text.nfc().collect::<String>());
        if let Some(defect) = defect_in(composed.as_deref().unwrap_or(&text)) {
            return Err(Error::MalformedEntryPath { path: text, defect });
        }
        Ok(Self(composed.unwrap_or(text)))
    }
}

impl FromStr for EntryPath {
    type Err = Error;

    /// [`parse`](Self::parse), reached the way a literal in a caller reads
    /// best: `"albums/a.jpg".parse()`.
    fn from_str(text: &str) -> Result<Self> {
        Self::parse(text)
    }
}
