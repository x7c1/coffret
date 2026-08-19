use anyhow::{Context, Result};
use coffret_model::{ContainerId, ContainerKey};
use serde::{Deserialize, Serialize};

use crate::hex;

use super::{EntryFixture, WireContainerKind};

/// One Container in a fixture set, with everything needed to open and check it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerFixture {
    /// The name this fixture is known by across both implementations.
    pub fixture: String,
    /// Where the bytes live, relative to the fixture directory.
    pub file: String,
    /// The name the object is stored under.
    pub object_name: String,
    /// The Container ID, as 32 lowercase hex characters.
    pub container_id: String,
    /// The Container Key, as 64 lowercase hex characters.
    pub container_key: String,
    /// Whether this Container is one-file or a Pack.
    pub kind: WireContainerKind,
    /// The chunk size the object was written with.
    pub chunk_size: u32,
    /// The entries the object must decode to, in plaintext stream order.
    pub entries: Vec<EntryFixture>,
}

impl ContainerFixture {
    /// The Container ID this fixture states.
    pub fn container_id(&self) -> Result<ContainerId> {
        Ok(ContainerId::from_bytes(
            hex::decode_array(&self.container_id).context("container_id")?,
        ))
    }

    /// The Container Key this fixture states.
    pub fn container_key(&self) -> Result<ContainerKey> {
        Ok(ContainerKey::from_bytes(
            hex::decode_array(&self.container_key).context("container_key")?,
        ))
    }
}
