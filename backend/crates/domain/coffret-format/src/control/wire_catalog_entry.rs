use coffret_model::{Btime, ContentHash, EntryMetadata, Mtime};
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::meta::{stored_path, WireDerivedFrom};
use crate::stream_extent::stream_extent;
use crate::wire_uint::WireUint;

/// One Entry as the catalog records it (FM-15, FM-16).
///
/// The same map as the meta section's `WireMetaEntry` — `offset`, `size`,
/// `hash`, `mime`, and `derived_from` are shared with it verbatim — except the
/// values a later rename could move. Those are spelled `path`, `mtime`, and
/// `btime` here, without the `original_` prefix FM-9 gives them, because a
/// record and a Snapshot are the catalog's durable form: an addition's values
/// are what the Library holds now, not what one immutable object happened to
/// capture. That is also why the two spellings are two structs rather than one
/// with serde renames: the sharing is of the values, not of the keys.
#[derive(Serialize, Deserialize)]
pub(crate) struct WireCatalogEntry {
    path: String,
    offset: WireUint,
    size: WireUint,
    mtime: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    btime: Option<i64>,
    #[serde(with = "serde_bytes")]
    hash: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    derived_from: Option<WireDerivedFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mime: Option<String>,
}

impl From<&EntryMetadata> for WireCatalogEntry {
    fn from(entry: &EntryMetadata) -> Self {
        Self {
            path: entry.path.as_str().to_owned(),
            offset: entry.extent.offset().into(),
            size: entry.extent.size().into(),
            mtime: entry.mtime.as_unix_seconds(),
            btime: entry.btime.map(|btime| btime.as_unix_seconds()),
            hash: entry.hash.as_bytes().to_vec(),
            derived_from: entry.derived_from.as_ref().map(WireDerivedFrom::from),
            mime: entry.mime.clone(),
        }
    }
}

impl WireCatalogEntry {
    pub(crate) fn to_metadata(&self) -> Result<EntryMetadata> {
        Ok(EntryMetadata {
            path: stored_path(&self.path, "path")?,
            extent: stream_extent(self.offset.get(), self.size.get())?,
            mtime: Mtime::from_unix_seconds(self.mtime),
            btime: self.btime.map(Btime::from_unix_seconds),
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
