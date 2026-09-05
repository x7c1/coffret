use bech32::primitives::decode::{
    CharError, CheckedHrpstring, CheckedHrpstringError, UncheckedHrpstringError,
};
use bech32::Bech32m;
use coffret_model::{MasterKey, MasterKeyEpoch};
use zeroize::Zeroizing;

use super::{offset, RecoveryCode};
use crate::error::{Error, Result};

/// How many padding bits [`RecoveryCode::DATA_LEN`] characters leave over 41
/// bytes: 66 × 5 − 41 × 8. KD-11 fixes them at zero, so a code whose last
/// character carries anything in them was not written by this form.
const PADDING_BITS: u8 = 0b11;

impl RecoveryCode {
    /// Reads a code a user wrote down, or refuses to read it at all.
    ///
    /// Whitespace and hyphens go first, so the grouped printing form and any
    /// other way the user broke the string up parse the same as the bare one;
    /// an entirely uppercase copy parses too, since Bech32 admits either case
    /// but not a mixture of them.
    ///
    /// Every remaining check either passes or ends the read naming itself, and
    /// none of them releases key material: a code with a mistyped character
    /// yields no Master Key rather than a different one (KD-11).
    pub fn parse(text: &str) -> Result<Self> {
        let normalized = normalize(text);
        let checked = CheckedHrpstring::new::<Bech32m>(normalized.as_str()).map_err(rejected)?;

        let hrp = checked.hrp();
        if hrp.to_lowercase() != Self::HUMAN_READABLE_PART {
            return Err(Error::UnknownRecoveryCodePrefix {
                actual: hrp.to_lowercase(),
            });
        }

        // The character count is the check rather than the byte count: 66
        // characters and 67 both yield 41 bytes, and only the first of them is
        // this form.
        let characters = checked.fe32_iter().count();
        if characters != Self::DATA_LEN {
            return Err(Error::RecoveryCodeLengthMismatch { actual: characters });
        }
        let last = checked
            .fe32_iter()
            .last()
            .expect("DATA_LEN characters is more than none");
        if last.to_u8() & PADDING_BITS != 0 {
            return Err(Error::NonZeroRecoveryCodePadding);
        }

        // The Master Key in the clear, once the checksum says the string is a
        // code at all. Wiped as this call returns, so the only copies that
        // outlive it are the ones inside the value it hands back (spec: DK-7).
        let payload = Zeroizing::new(checked.byte_iter().collect::<Vec<u8>>());
        debug_assert_eq!(payload.len(), Self::PAYLOAD_LEN);

        let version = payload[offset::VERSION];
        if version != Self::VERSION {
            return Err(Error::UnsupportedRecoveryCodeVersion { actual: version });
        }
        // The 8 bytes spell any `u64`, and the ones that number an epoch run
        // from 1 to the largest integer the format admits (FM-13, FM-19). The
        // rule is the format's, so the refusal is this layer's rather than the
        // model's passed through — the reading the control header's generation
        // already gets.
        let number = u64::from_be_bytes(
            payload[offset::EPOCH]
                .try_into()
                .expect("the slice is 8 bytes long"),
        );
        let epoch = MasterKeyEpoch::new(number)
            .map_err(|_| Error::RecoveryCodeEpochOutOfRange { epoch: number })?;
        let master_key = MasterKey::from_bytes(
            payload[offset::MASTER_KEY..]
                .try_into()
                .expect("the slice is MasterKey::BYTE_LEN long"),
        );

        // Re-encoded rather than kept: the canonical spelling of a code is the
        // lowercase one, whichever case and grouping it arrived in.
        Ok(Self::encode(master_key, epoch))
    }
}

/// Drops what a person adds writing a code down by hand.
///
/// What comes back is the whole code, which is the Master Key in another
/// spelling, so it is wiped when the read that built it ends rather than left in
/// freed memory — including when a check further down refuses the string. The
/// buffer is drawn at the input's length so that growing it copies nothing
/// half-built into a second allocation the wipe would never reach (spec: DK-7).
fn normalize(text: &str) -> Zeroizing<String> {
    let mut normalized = String::with_capacity(text.len());
    normalized.extend(
        text.chars()
            .filter(|character| !character.is_ascii_whitespace() && *character != '-'),
    );
    Zeroizing::new(normalized)
}

/// Names the check the string failed before its payload was ever reached.
fn rejected(error: CheckedHrpstringError) -> Error {
    match error {
        CheckedHrpstringError::Checksum(_) => Error::RecoveryCodeChecksumFailed,
        CheckedHrpstringError::Parse(UncheckedHrpstringError::Char(CharError::MixedCase)) => {
            Error::RecoveryCodeMixedCase
        }
        CheckedHrpstringError::Parse(UncheckedHrpstringError::Char(CharError::InvalidChar(
            actual,
        ))) => Error::RecoveryCodeInvalidCharacter { actual },
        // What is left is a string with no `1` to divide at, nothing before the
        // one it has, or a prefix that is not characters a human-readable part
        // may be built from: not a code with something wrong in it, but not a
        // code at all. A string cut short after its separator is not among them
        // — too few characters to checksum is a checksum failure above.
        _ => Error::MalformedRecoveryCode,
    }
}
