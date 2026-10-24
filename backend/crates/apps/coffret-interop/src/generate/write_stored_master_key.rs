use coffret_format::{Argon2Params, StoredMasterKey};
use coffret_model::{MasterKey, MasterKeyEpoch, Passphrase};

use crate::fixture_set::{FixtureWriter, BLOBS_DIR};
use crate::manifest::{Argon2ParamsFixture, StoredMasterKeyFixture};
use crate::{hex, Error, Result, Step};

use super::{EPOCH, PASSPHRASE};

pub(super) fn write_stored_master_key(
    writer: &FixtureWriter,
    master_key: &MasterKey,
) -> Result<StoredMasterKeyFixture> {
    let params = Argon2Params::INITIAL;
    let epoch = MasterKeyEpoch::new(EPOCH).map_err(Error::model(Step::StatingGeneratorValue))?;
    let stored = StoredMasterKey::create_with(
        params,
        &Passphrase::from_bytes(PASSPHRASE.as_bytes().to_vec()),
        master_key,
        epoch,
    )
    .map_err(Error::format(Step::ProtectingMasterKey))?;

    let file = writer.write(BLOBS_DIR, "stored-master-key.bin", stored.as_bytes())?;
    Ok(StoredMasterKeyFixture {
        fixture: "stored-master-key".to_owned(),
        file,
        master_key: hex::encode(master_key.as_bytes()),
        epoch: epoch.get(),
        argon2: Argon2ParamsFixture::from(params),
    })
}
