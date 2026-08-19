use coffret_model::{MasterKey, MasterKeyEpoch};

use super::{offset, Argon2Params, StoredMasterKey, PLAINTEXT_LEN};
use crate::aead::Cipher;
use crate::entropy;
use crate::error::Result;
use crate::nonce;

impl StoredMasterKey {
    /// Wraps a Master Key under a Passphrase at this build's initial cost.
    pub fn create(
        passphrase: &[u8],
        master_key: &MasterKey,
        epoch: MasterKeyEpoch,
    ) -> Result<Self> {
        Self::create_with(Argon2Params::INITIAL, passphrase, master_key, epoch)
    }

    /// Wraps a Master Key under a Passphrase at a stated cost.
    ///
    /// The salt is drawn here rather than taken from the caller: it is per device
    /// and per wrap, and nothing outside this module needs to choose it.
    pub fn create_with(
        params: Argon2Params,
        passphrase: &[u8],
        master_key: &MasterKey,
        epoch: MasterKeyEpoch,
    ) -> Result<Self> {
        let salt: [u8; Self::SALT_LEN] = entropy::draw()?;
        let nonce = nonce::random()?;

        let mut bytes = Vec::with_capacity(offset::SALT + salt.len() + nonce::LEN + PLAINTEXT_LEN);
        bytes.extend_from_slice(&Self::MAGIC);
        bytes.push(Self::VERSION);
        bytes.push(0); // reserved
        bytes.push(u8::try_from(salt.len()).expect("this build's salt is shorter than 256 bytes"));
        bytes.extend_from_slice(&params.memory_kib().to_be_bytes());
        bytes.extend_from_slice(&params.iterations().to_be_bytes());
        bytes.extend_from_slice(&params.parallelism().to_be_bytes());
        bytes.extend_from_slice(&salt);
        bytes.extend_from_slice(&nonce);

        let mut plaintext = Vec::with_capacity(PLAINTEXT_LEN);
        plaintext.extend_from_slice(master_key.as_bytes());
        plaintext.extend_from_slice(&epoch.get().to_be_bytes());

        // Everything written so far — the parameters, the salt, and the nonce —
        // is the associated data, which is what makes a parameter downgrade
        // detectable rather than merely useless.
        let associated_data = bytes.clone();
        let protection_key = params.derive(passphrase, &salt)?;
        Cipher::new(&protection_key).seal(&nonce, &associated_data, &mut plaintext, &mut bytes)?;
        Self::from_bytes(bytes)
    }
}
