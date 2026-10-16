use coffret_model::{Btime, ContentHash, EntryMetadata, Mtime};
use serde::{Deserialize, Serialize};

use crate::bounded_uint::bounded_uint;
use crate::error::{Error, Result};
use crate::meta::{stored_path, WireDerivedFrom};
use crate::stream_extent::stream_extent;

/// One Entry as the catalog records it (spec: FM-15, FM-16).
///
/// The same map as the meta section's `WireMetaEntry` — `offset`, `size`,
/// `hash`, `mime`, and `derived_from` are shared with it verbatim — except the
/// values a later rename could move. Those are spelled `path`, `mtime`, and
/// `btime` here, without the `original_` prefix FM-9 gives them, because a
/// record and a Snapshot are the catalog's durable form: an addition's values
/// are what the Library holds now, not what one immutable object happened to
/// capture. That is also why the two spellings are two structs rather than one
/// with serde renames: the sharing is of the values, not of the keys.
///
/// The name is the one value that can be carried in both spellings: `path` is
/// where the Entry stands in the Library now, and `original_path` is the Entry
/// Path the owning Container's meta section records — the name it was captured
/// under, which no rename moves. `original_path` is written only where it
/// differs from `path`, so a map without it names an Entry whose two paths are
/// one, which is also what every map written before the field existed means. A
/// map carrying it equal to `path` is a second spelling of that same state and
/// is refused.
#[derive(Serialize, Deserialize)]
pub(crate) struct WireCatalogEntry {
    path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    original_path: Option<String>,
    offset: u64,
    size: u64,
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
            // Present only where a rename has set the two apart (spec: FM-15,
            // FM-16), so one state has one encoding.
            original_path: (entry.original_path != entry.path)
                .then(|| entry.original_path.as_str().to_owned()),
            offset: entry.extent.offset(),
            size: entry.extent.size(),
            mtime: entry.mtime.as_unix_seconds(),
            btime: entry.btime.map(|btime| btime.as_unix_seconds()),
            hash: entry.hash.as_bytes().to_vec(),
            derived_from: entry.derived_from.as_ref().map(WireDerivedFrom::from),
            mime: entry.mime.clone(),
        }
    }
}

impl WireCatalogEntry {
    /// This row as the Entry it records, or the refusal of whichever payload
    /// carried it.
    ///
    /// The same map travels in a Journal record and in an Index Snapshot, and a
    /// row neither of them could have written is that object's own malformed
    /// payload rather than one shared verdict — which is what `malformed` says
    /// here, exactly as it does everywhere else those two schemas are read.
    pub(crate) fn to_metadata(&self, malformed: fn(String) -> Error) -> Result<EntryMetadata> {
        let path = stored_path(&self.path, "path")?;
        // Absent where the two paths are one, which every map an earlier
        // schema wrote also is (spec: FM-15, FM-16).
        let original_path = match &self.original_path {
            Some(original_path) => {
                let original_path = stored_path(original_path, "original_path")?;
                if original_path == path {
                    return Err(malformed(
                        "an entry map carries an `original_path` equal to its `path`, \
                         which is written by leaving it out"
                            .to_owned(),
                    ));
                }
                original_path
            }
            None => path.clone(),
        };
        Ok(EntryMetadata {
            path,
            original_path,
            extent: stream_extent(
                bounded_uint("offset", self.offset, malformed)?,
                bounded_uint("size", self.size, malformed)?,
            )?,
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
