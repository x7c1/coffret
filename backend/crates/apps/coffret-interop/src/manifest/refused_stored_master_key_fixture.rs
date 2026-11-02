use serde::{Deserialize, Serialize};

/// One byte string that is not a stored Master Key form a reader accepts, and
/// the refusal reading it must give.
///
/// A reader checks a form in the order KD-9 states and refuses it for the
/// first fault in that order, so a form with several faults is refused with
/// one kind in every implementation. The fixture carries such a form, so two
/// readers that check in different orders disagree here rather than on a
/// device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefusedStoredMasterKeyFixture {
    /// The name this fixture is known by across both implementations.
    pub fixture: String,
    /// Where the bytes live, relative to the fixture directory.
    pub file: String,
    /// The refusal reading the bytes must give.
    pub refusal: StoredMasterKeyRefusal,
}

/// A reader's refusal of a stored Master Key form, spelled as the TypeScript
/// implementation's error code spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StoredMasterKeyRefusal {
    /// The form ends early, or bytes follow it.
    StoredMasterKeyLengthMismatch,
    /// The leading bytes are not the stored Master Key magic.
    UnknownStoredMasterKeyMagic,
    /// The version byte names a form this build cannot read.
    UnsupportedStoredMasterKeyVersion,
    /// The reserved byte is not zero.
    ReservedNotZero,
    /// The recorded cost is one Argon2id itself refuses.
    InvalidArgon2Params,
    /// The recorded cost is above a ceiling KD-9 sets.
    Argon2CostAboveCeiling,
}

impl StoredMasterKeyRefusal {
    /// The refusal the format crate's `error` is, where it is a refusal of a
    /// stored form's bytes at all.
    pub fn of(error: &coffret_format::Error) -> Option<Self> {
        use coffret_format::Error;
        match error {
            Error::StoredMasterKeyLengthMismatch => Some(Self::StoredMasterKeyLengthMismatch),
            Error::UnknownStoredMasterKeyMagic { .. } => Some(Self::UnknownStoredMasterKeyMagic),
            Error::UnsupportedStoredMasterKeyVersion { .. } => {
                Some(Self::UnsupportedStoredMasterKeyVersion)
            }
            Error::ReservedNotZero => Some(Self::ReservedNotZero),
            Error::InvalidArgon2Params { .. } => Some(Self::InvalidArgon2Params),
            Error::Argon2CostAboveCeiling { .. } => Some(Self::Argon2CostAboveCeiling),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Both implementations state the refusal, so the manifest spells it as the
    // TypeScript error code does.
    #[test]
    fn the_refusal_spellings_are_the_typescript_codes() {
        assert_eq!(
            serde_json::to_string(&StoredMasterKeyRefusal::StoredMasterKeyLengthMismatch).unwrap(),
            r#""stored_master_key_length_mismatch""#
        );
        assert_eq!(
            serde_json::to_string(&StoredMasterKeyRefusal::Argon2CostAboveCeiling).unwrap(),
            r#""argon2_cost_above_ceiling""#
        );
        assert_eq!(
            serde_json::to_string(&StoredMasterKeyRefusal::InvalidArgon2Params).unwrap(),
            r#""invalid_argon2_params""#
        );
    }
}
