use anyhow::{Context, Result};
use coffret_model::DerivedFrom;
use serde::{Deserialize, Serialize};

use crate::hex;

use super::DerivedFromFixture;

/// One Entry a Container must decode to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntryFixture {
    /// The Library position this Entry occupies.
    pub path: String,
    /// The modification time, as whole seconds from the Unix epoch.
    pub mtime: i64,
    /// The birth time, as whole seconds from the Unix epoch, where the writer's
    /// platform reported one.
    ///
    /// Absent means the Container records none — never "created at the epoch" —
    /// so a reader that filled it in would fail the exchange.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub btime: Option<i64>,
    /// The Entry's plaintext, as lowercase hex.
    pub content: String,
    /// Set when this Entry holds data derived from another Entry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub derived_from: Option<DerivedFromFixture>,
    /// The media type of the content, when the writer recorded one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime: Option<String>,
}

impl EntryFixture {
    /// The Entry's plaintext.
    pub fn content(&self) -> Result<Vec<u8>> {
        hex::decode(&self.content).context("content")
    }

    /// The parent this Entry was derived from, if the fixture states one.
    pub fn derived_from(&self) -> Result<Option<DerivedFrom>> {
        self.derived_from
            .as_ref()
            .map(DerivedFromFixture::to_model)
            .transpose()
    }
}
