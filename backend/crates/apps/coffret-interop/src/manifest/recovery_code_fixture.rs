use anyhow::{Context, Result};
use coffret_model::{MasterKey, MasterKeyEpoch};
use serde::{Deserialize, Serialize};

use crate::hex;

/// One Recovery Code in a fixture set, and the pair reading it must give.
///
/// The code is text rather than an opaque byte string, so the file holds the
/// characters a user would have written down — the whitespace and case of the
/// file included, since both are part of what a reader has to take (KD-11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryCodeFixture {
    /// The name this fixture is known by across both implementations.
    pub fixture: String,
    /// Where the code's characters live, relative to the fixture directory.
    pub file: String,
    /// The Master Key reading it must yield, as 64 lowercase hex characters.
    pub master_key: String,
    /// The epoch that key belongs to.
    pub epoch: u64,
}

impl RecoveryCodeFixture {
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
