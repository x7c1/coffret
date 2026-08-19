use anyhow::{Context, Result};
use coffret_model::{MasterKey, MasterKeyEpoch};
use serde::{Deserialize, Serialize};

use crate::hex;

use super::Argon2ParamsFixture;

/// One stored Master Key form in a fixture set, and what unlocking it must give.
///
/// The form never reaches Storage (KD-8); it is here because it is the one
/// Passphrase-derived byte string two implementations must agree on for a
/// device to move between them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredMasterKeyFixture {
    /// The name this fixture is known by across both implementations.
    pub fixture: String,
    /// Where the bytes live, relative to the fixture directory.
    pub file: String,
    /// The Master Key unlocking must yield, as 64 lowercase hex characters.
    pub master_key: String,
    /// The epoch that key belongs to.
    pub epoch: u64,
    /// The Argon2id cost the form was written at.
    pub argon2: Argon2ParamsFixture,
}

impl StoredMasterKeyFixture {
    /// The Master Key this fixture states.
    pub fn master_key(&self) -> Result<MasterKey> {
        Ok(MasterKey::from_bytes(
            hex::decode_array(&self.master_key).context("master_key")?,
        ))
    }

    /// The epoch this fixture states.
    pub fn epoch(&self) -> Result<MasterKeyEpoch> {
        Ok(MasterKeyEpoch::new(self.epoch)?)
    }
}
