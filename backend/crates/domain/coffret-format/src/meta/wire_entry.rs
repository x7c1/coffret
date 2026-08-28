use coffret_model::{ContentHash, EntryMetadata, Mtime};
use serde::{Deserialize, Serialize};

use super::stored_path::stored_path;
use super::wire_derived_from::WireDerivedFrom;
use crate::error::Result;

#[derive(Serialize, Deserialize)]
pub(crate) struct WireEntry {
    path: String,
    offset: u64,
    size: u64,
    mtime: i64,
    #[serde(with = "serde_bytes")]
    hash: Vec<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    derived_from: Option<WireDerivedFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mime: Option<String>,
}

impl From<&EntryMetadata> for WireEntry {
    fn from(entry: &EntryMetadata) -> Self {
        Self {
            path: entry.path.as_str().to_owned(),
            offset: entry.offset,
            size: entry.size,
            mtime: entry.mtime.as_unix_seconds(),
            hash: entry.hash.as_bytes().to_vec(),
            derived_from: entry.derived_from.as_ref().map(WireDerivedFrom::from),
            mime: entry.mime.clone(),
        }
    }
}

impl WireEntry {
    pub(crate) fn to_metadata(&self) -> Result<EntryMetadata> {
        Ok(EntryMetadata {
            path: stored_path(&self.path, "path")?,
            offset: self.offset,
            size: self.size,
            mtime: Mtime::from_unix_seconds(self.mtime),
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
