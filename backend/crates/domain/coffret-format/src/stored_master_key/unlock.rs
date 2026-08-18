use coffret_model::{MasterKey, MasterKeyEpoch};

use super::{StoredMasterKey, UnlockedMasterKey};
use crate::aead::Cipher;
use crate::error::Result;
use crate::nonce;

impl StoredMasterKey {
    /// Opens the form with the Passphrase that wrapped it.
    ///
    /// The derivation follows the parameters recorded in these bytes rather than
    /// this build's current policy, so a form written before a device raised its
    /// cost still unlocks — and a form whose recorded cost was edited fails, as
    /// the parameters are authenticated.
    pub fn unlock(&self, passphrase: &[u8]) -> Result<UnlockedMasterKey> {
        let layout = &self.layout;
        let protection_key = layout
            .params
            .derive(passphrase, &self.bytes[layout.salt.clone()])?;
        let nonce: [u8; nonce::LEN] = self.bytes[layout.nonce.clone()]
            .try_into()
            .expect("the slice is nonce::LEN long");

        let plaintext = Cipher::new(&protection_key).open(
            &nonce,
            &self.bytes[..layout.message.start],
            &self.bytes[layout.message.clone()],
        )?;
        let (master_key, epoch) = plaintext.split_at(MasterKey::BYTE_LEN);

        Ok(UnlockedMasterKey {
            master_key: MasterKey::from_bytes(
                master_key
                    .try_into()
                    .expect("the slice is MasterKey::BYTE_LEN long"),
            ),
            epoch: MasterKeyEpoch::new(u64::from_be_bytes(
                epoch.try_into().expect("the slice is 8 bytes long"),
            ))?,
        })
    }
}
