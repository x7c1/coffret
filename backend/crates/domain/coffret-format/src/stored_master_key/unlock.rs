use coffret_model::{MasterKey, MasterKeyEpoch, Passphrase};
use zeroize::Zeroizing;

use super::{StoredMasterKey, UnlockedMasterKey};
use crate::aead::Cipher;
use crate::error::Result;
use crate::nonce;

impl StoredMasterKey {
    /// Opens the form with the Passphrase that protects it.
    ///
    /// The derivation follows the parameters recorded in these bytes rather than
    /// this build's current policy, so a form written before a device raised its
    /// cost still unlocks — and a form whose recorded cost was edited fails, as
    /// the parameters are authenticated.
    pub fn unlock(&self, passphrase: &Passphrase) -> Result<UnlockedMasterKey> {
        let layout = &self.layout;
        let protection_key = layout
            .params
            .derive(passphrase, &self.bytes[layout.salt.clone()])?;
        let nonce: [u8; nonce::LEN] = self.bytes[layout.nonce.clone()]
            .try_into()
            .expect("the slice is nonce::LEN long");

        // The Master Key in the clear, for as long as it takes to read the two
        // fields out of it. The buffer is owned here and wiped when this call
        // ends, so the only copy that outlives the call is the one inside
        // `MasterKey` — which wipes itself in turn (spec: DK-7).
        let plaintext = Zeroizing::new(Cipher::new(&protection_key).open(
            &nonce,
            &self.bytes[..layout.message.start],
            &self.bytes[layout.message.clone()],
        )?);
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
