use coffret_format::StoredMasterKey;

use crate::fixture_set::FixtureReader;
use crate::manifest::{RefusedStoredMasterKeyFixture, StoredMasterKeyRefusal};
use crate::{Error, Result, Step};

use super::same;

/// Checks that reading the fixture's bytes is the refusal the manifest states.
///
/// A form with several faults is refused for the first of them in KD-9's
/// order, so a reader that checks in another order gives another refusal and
/// fails here.
pub(super) fn check_refused_stored_master_key(
    reader: &FixtureReader,
    fixture: &RefusedStoredMasterKeyFixture,
) -> Result<()> {
    let bytes = reader.read(&fixture.file)?;
    let refused = match StoredMasterKey::from_bytes(bytes) {
        Ok(_) => {
            return Err(Error::NotRefused {
                stated: fixture.refusal,
            })
        }
        Err(refused) => refused,
    };
    // A failure that is no refusal of the form's bytes is the format crate's
    // own, and travels as it reported it.
    let Some(decoded) = StoredMasterKeyRefusal::of(&refused) else {
        return Err(Error::Format {
            step: Step::ReadingStoredMasterKey,
            cause: refused,
        });
    };
    same("refusal", &decoded, &fixture.refusal)
}
