use anyhow::{Context, Result};
use coffret_format::{Argon2Params, StoredMasterKey};
use coffret_model::{MasterKey, MasterKeyEpoch};

use crate::fixture_set::{FixtureWriter, BLOBS_DIR};
use crate::hex;
use crate::manifest::{Argon2ParamsFixture, StoredMasterKeyFixture};

use super::{EPOCH, PASSPHRASE};

pub(super) fn write_stored_master_key(
    writer: &FixtureWriter,
    master_key: &MasterKey,
) -> Result<StoredMasterKeyFixture> {
    let params = Argon2Params::INITIAL;
    let epoch = MasterKeyEpoch::new(EPOCH)?;
    let stored = StoredMasterKey::create_with(params, PASSPHRASE.as_bytes(), master_key, epoch)
        .context("protecting the Master Key under the Passphrase")?;

    let file = writer.write(BLOBS_DIR, "stored-master-key.bin", stored.as_bytes())?;
    Ok(StoredMasterKeyFixture {
        fixture: "stored-master-key".to_owned(),
        file,
        master_key: hex::encode(master_key.as_bytes()),
        epoch: epoch.get(),
        argon2: Argon2ParamsFixture::from(params),
    })
}
