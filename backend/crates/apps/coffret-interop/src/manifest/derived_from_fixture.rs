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
    pub(super) fn to_model(&self) -> Result<DerivedFrom> {
        Ok(DerivedFrom {
            container_id: ContainerId::from_bytes(
                hex::decode_array(&self.container_id).context("derived_from.container_id")?,
            ),
            path: EntryPath::new(self.path.clone()),
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
