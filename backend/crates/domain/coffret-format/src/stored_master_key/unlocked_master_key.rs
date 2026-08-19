use coffret_model::{MasterKey, MasterKeyEpoch};

/// What a stored Master Key yields once the Passphrase opens it.
///
/// The epoch travels with the key because a key alone does not say which epoch
/// it belongs to, and every control object the device reads names the epoch that
/// wrote it.
#[derive(Debug, Clone)]
pub struct UnlockedMasterKey {
    /// The Master Key this device holds.
    pub master_key: MasterKey,
    /// The epoch that key belongs to.
    pub epoch: MasterKeyEpoch,
}
