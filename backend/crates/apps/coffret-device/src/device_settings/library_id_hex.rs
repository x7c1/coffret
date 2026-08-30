//! The Library ID as the settings file spells it.
//!
//! `coffret-model` takes no third-party dependency at all, so no domain type
//! carries a `serde` implementation and the spelling has to be given here. The
//! spelling is the one every identifier in coffret is written in — 16 lowercase
//! hex characters — so a Library ID read out of this file and one read off an
//! app folder's name are the same string (spec: FM-18).

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serializer};

use coffret_model::LibraryId;

/// Writes the ID as its hex spelling.
pub(super) fn serialize<S>(id: &LibraryId, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&id.to_hex())
}

/// Reads the hex spelling back, refusing anything that is not one.
///
/// A refusal here becomes the settings file being unreadable, which is the
/// honest verdict: a file whose Library ID cannot be read names no Library, and
/// no part of it may be acted on.
pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<LibraryId, D::Error>
where
    D: Deserializer<'de>,
{
    let hex = String::deserialize(deserializer)?;
    LibraryId::from_hex(&hex).map_err(D::Error::custom)
}
