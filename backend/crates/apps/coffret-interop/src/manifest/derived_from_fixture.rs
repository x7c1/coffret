use anyhow::{Context, Result};
use coffret_model::{ContainerId, DerivedFrom, EntryPath};
use serde::{Deserialize, Serialize};

use crate::hex;

/// The parent Entry a derived Entry points at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DerivedFromFixture {
    /// The Container holding the parent Entry, as 32 lowercase hex characters.
    pub container_id: String,
    /// The parent Entry's path.
    pub path: String,
}

impl DerivedFromFixture {
    /// The parent this fixture states, in the domain's own vocabulary.
    ///
    /// The path is taken as the manifest spells it and never composed. What
    /// this value is for is a comparison against what a Container decoded to,
    /// and the Entry's own path is compared as bytes; composing this one would
    /// let a manifest spelling a path one way and a Container spelling it
    /// another pass for agreement (spec: EP-1, EP-3). A manifest that is not
    /// NFC is a manifest no implementation holding to EP-1 wrote, and it is
    /// reported rather than quietly read past.
    pub(super) fn to_model(&self) -> Result<DerivedFrom> {
        Ok(DerivedFrom {
            container_id: ContainerId::from_bytes(
                hex::decode_array(&self.container_id).context("derived_from.container_id")?,
            ),
            path: EntryPath::stored(self.path.clone()).context("derived_from.path")?,
        })
    }

    /// States the parent a decoded Entry points at.
    pub fn from_model(derived_from: &DerivedFrom) -> Self {
        Self {
            container_id: hex::encode(derived_from.container_id.as_bytes()),
            path: derived_from.path.as_str().to_owned(),
        }
    }
}
