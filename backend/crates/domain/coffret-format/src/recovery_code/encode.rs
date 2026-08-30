use bech32::Bech32m;
use coffret_model::{MasterKey, MasterKeyEpoch};

use super::{offset, RecoveryCode, HRP};

impl RecoveryCode {
    /// Writes a Master Key and its epoch as the code their owner keeps.
    ///
    /// The epoch travels with the key because a key alone does not say which
    /// control objects on Storage it opens (KD-11).
    pub fn encode(master_key: &MasterKey, epoch: MasterKeyEpoch) -> Self {
        let mut payload = [0u8; Self::PAYLOAD_LEN];
        payload[offset::VERSION] = Self::VERSION;
        payload[offset::EPOCH].copy_from_slice(&epoch.get().to_be_bytes());
        payload[offset::MASTER_KEY..].copy_from_slice(master_key.as_bytes());

        // The only failure `encode_lower` reports is a code past Bech32's
        // 90-character limit, and this payload is a fixed 41 bytes: every
        // Recovery Code is 80 characters.
        let text = bech32::encode_lower::<Bech32m>(HRP, &payload)
            .expect("a 41-byte payload is inside Bech32's length limit");

        Self {
            master_key: master_key.clone(),
            epoch,
            text,
        }
    }
}
