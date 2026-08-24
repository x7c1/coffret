use coffret_format::{Purpose, PurposeKey};
use coffret_model::{MasterKey, MasterKeyEpoch};

use crate::fetch::LibraryKeys;

/// The Master Key both devices are enrolled under.
fn master_key() -> MasterKey {
    MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN])
}

/// Everything one epoch's Containers are sealed and opened with.
pub(crate) fn keys() -> LibraryKeys {
    LibraryKeys::derive(&master_key(), MasterKeyEpoch::FIRST)
}

/// The purpose key one kind of object is sealed under (spec: KD-4).
///
/// The cases derive their own rather than borrowing a run's, so that what they
/// open or write a control object with is the rule and not the code under test.
pub(super) fn purpose_key(purpose: Purpose) -> PurposeKey {
    PurposeKey::derive(&master_key(), purpose)
}
