use anyhow::{Context, Result};
use coffret_model::{ContainerId, ContainerKey};
use serde::{Deserialize, Serialize};

use crate::hex;

/// One Key Envelope in a fixture set, and the key it must unwrap to.
///
/// The envelope is not a Storage Object of its own — it lives inside a Keyring
/// — so it travels as a plain blob beside the objects rather than under an
/// object name.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyEnvelopeFixture {
    /// The name this fixture is known by across both implementations.
    pub fixture: String,
    /// Where the 72 bytes live, relative to the fixture directory.
    pub file: String,
    /// The Container the envelope is bound to, as 32 lowercase hex characters.
    pub container_id: String,
    /// The Container Key the envelope must unwrap to, as 64 hex characters.
    pub container_key: String,
}

impl KeyEnvelopeFixture {
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
