use coffret_format::StoredMasterKey;
use coffret_model::Passphrase;

use crate::fixture_set::FixtureReader;
use crate::manifest::{Argon2ParamsFixture, StoredMasterKeyFixture};
use crate::{hex, Error, Result, Step};

use super::same;

pub(super) fn check_stored_master_key(
    reader: &FixtureReader,
    passphrase: &Passphrase,
    fixture: &StoredMasterKeyFixture,
) -> Result<()> {
    let bytes = reader.read(&fixture.file)?;
    let stored =
        StoredMasterKey::from_bytes(bytes).map_err(Error::format(Step::ReadingStoredMasterKey))?;
    // A reader follows the cost the form records rather than its own policy, so
    // the recorded cost is itself an expectation the manifest states.
    same(
        "argon2",
        &Argon2ParamsFixture::from(stored.params()),
        &fixture.argon2,
    )?;

    let unlocked = stored
        .unlock(passphrase)
        .map_err(Error::format(Step::UnlockingMasterKey))?;
    same(
        "master_key",
        &hex::encode(unlocked.master_key.as_bytes()),
        &hex::encode(fixture.master_key()?.as_bytes()),
    )?;
    same("epoch", &unlocked.epoch, &fixture.epoch()?)
}
