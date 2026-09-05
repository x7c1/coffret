use coffret_model::{Btime, ContentHash, EntryMetadata, Mtime};
use serde::{Deserialize, Serialize};

use super::malformed;
use super::stored_path::stored_path;
use super::wire_derived_from::WireDerivedFrom;

use crate::bounded_uint::bounded_uint;
use crate::error::Result;
use crate::stream_extent::stream_extent;

/// One row of a Container's entry table, in the meta section's spelling (FM-9).
///
/// The three values a later rename could move carry `original_` names here:
/// what the Entry was called, when it was last modified, and when its file came
/// into being *as of the moment this Container was written* — not the first
/// name the Entry ever had. A Container is one immutable object, so nothing
/// rewrites them; what the Library holds now is the Journal's business, and a
/// record spells the same values `WireCatalogEntry`'s way.
#[derive(Serialize, Deserialize)]
pub(crate) struct WireMetaEntry {
    original_path: String,
    offset: u64,
    size: u64,
    original_mtime: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    original_btime: Option<i64>,
    #[serde(with = "serde_bytes")]
    hash: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    derived_from: Option<WireDerivedFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mime: Option<String>,
}

impl From<&EntryMetadata> for WireMetaEntry {
    fn from(entry: &EntryMetadata) -> Self {
        Self {
            original_path: entry.path.as_str().to_owned(),
            offset: entry.extent.offset(),
            size: entry.extent.size(),
            original_mtime: entry.mtime.as_unix_seconds(),
            original_btime: entry.btime.map(|btime| btime.as_unix_seconds()),
            hash: entry.hash.as_bytes().to_vec(),
            derived_from: entry.derived_from.as_ref().map(WireDerivedFrom::from),
            mime: entry.mime.clone(),
        }
    }
}

impl WireMetaEntry {
    pub(crate) fn to_metadata(&self) -> Result<EntryMetadata> {
        Ok(EntryMetadata {
            path: stored_path(&self.original_path, "original_path")?,
            extent: stream_extent(
                bounded_uint("offset", self.offset, malformed)?,
                bounded_uint("size", self.size, malformed)?,
            )?,
            mtime: Mtime::from_unix_seconds(self.original_mtime),
            btime: self.original_btime.map(Btime::from_unix_seconds),
            hash: ContentHash::from_slice(&self.hash)?,
            derived_from: self
                .derived_from
                .as_ref()
                .map(WireDerivedFrom::to_domain)
                .transpose()?,
            mime: self.mime.clone(),
        })
    }
}
