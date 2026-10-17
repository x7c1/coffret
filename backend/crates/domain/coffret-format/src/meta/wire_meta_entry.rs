use coffret_model::{Btime, ContentHash, EntryMetadata, Mtime};
use serde::{Deserialize, Serialize};

use super::malformed;
use super::stored_path::stored_path;
use super::wire_derived_from::WireDerivedFrom;

use crate::bounded_uint::bounded_uint;
use crate::error::Result;
use crate::stream_extent::stream_extent;

/// One row of a Container's entry table, under the meta section's field names
/// (spec: FM-9).
///
/// The three values the Library may later hold differently carry `captured_`
/// names here: what the Entry was called, when it was last modified, and when
/// its file came into being *as captured when this Container was written* — not
/// the first name the Entry ever had. A Container is one immutable object, so
/// nothing rewrites them; what the Library holds now is the Journal's business,
/// and a record names the same values `WireCatalogEntry`'s way. The names are
/// the specification's; on the wire each travels under its field number
/// ([`crate::field_numbers::META_ENTRY`]).
#[derive(Serialize, Deserialize)]
pub(crate) struct WireMetaEntry {
    captured_path: String,
    offset: u64,
    size: u64,
    captured_mtime: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    captured_btime: Option<i64>,
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
            captured_path: entry.captured_path.as_str().to_owned(),
            offset: entry.extent.offset(),
            size: entry.extent.size(),
            captured_mtime: entry.mtime.as_unix_seconds(),
            captured_btime: entry.btime.map(|btime| btime.as_unix_seconds()),
            hash: entry.hash.as_bytes().to_vec(),
            derived_from: entry.derived_from.as_ref().map(WireDerivedFrom::from),
            mime: entry.mime.clone(),
        }
    }
}

impl WireMetaEntry {
    pub(crate) fn to_metadata(&self) -> Result<EntryMetadata> {
        let captured_path = stored_path(&self.captured_path, "captured_path")?;
        Ok(EntryMetadata {
            // What this Container records is the name the Entry had when it was
            // written, and that is all a meta section can say about where it
            // stands.
            path: captured_path.clone(),
            captured_path,
            extent: stream_extent(
                bounded_uint("offset", self.offset, malformed)?,
                bounded_uint("size", self.size, malformed)?,
            )?,
            mtime: Mtime::from_unix_seconds(self.captured_mtime),
            btime: self.captured_btime.map(Btime::from_unix_seconds),
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
