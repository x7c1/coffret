use bech32::primitives::checksum::Engine;
use bech32::primitives::decode::{
    CharError, CheckedHrpstring, CheckedHrpstringError, UncheckedHrpstringError,
};
use bech32::{Bech32m, Checksum, Fe32};
use coffret_model::{Error as ModelError, MasterKey, MasterKeyEpoch};
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
        divide(normalized.as_str())?;
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
        let epoch = MasterKeyEpoch::new(number).map_err(epoch_refusal)?;
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

/// The model's refusal to number an epoch as this carrier states it.
///
/// The offending number is carried by the model's own variant and restated in
/// this crate's, so a caller reading a code back gets the code's refusal rather
/// than the model's — the reading the control header's generation already gets.
/// Anything else the model refuses an epoch for would be a rule this layer has
/// not been told about, and passing it through says so.
fn epoch_refusal(error: ModelError) -> Error {
    match error {
        ModelError::EpochOutOfRange { epoch } => Error::RecoveryCodeEpochOutOfRange { epoch },
        other => Error::Model(other),
    }
}

/// The separator Bech32 divides a string at: the last `1` in it, since a
/// human-readable part may hold the character and a data part may not.
const SEPARATOR: char = '1';

/// The longest human-readable part Bech32 lets a string have.
const MAX_PREFIX_LEN: usize = 83;

/// Makes the checks KD-11 makes before the alphabet, in the order it makes
/// them, each ending the read with its own refusal.
///
/// The `bech32` crate makes these same checks, but in an order of its own: it
/// runs the alphabet over everything after the last `1` first, and over the
/// whole string when there is no `1` at all — so `coffretqqqq` would come back
/// as a character outside the alphabet rather than as the missing separator it
/// is. Making them here first gives the person the answer that says what to
/// change, and gives the same answer the TypeScript reader gives.
fn divide(text: &str) -> Result<()> {
    let upper = text.chars().any(|character| character.is_ascii_uppercase());
    let lower = text.chars().any(|character| character.is_ascii_lowercase());
    if upper && lower {
        return Err(Error::RecoveryCodeMixedCase);
    }
    let Some(separator) = text.rfind(SEPARATOR) else {
        return Err(Error::RecoveryCodeMissingSeparator);
    };
    let prefix = &text[..separator];
    if prefix.is_empty() {
        return Err(Error::RecoveryCodeEmptyPrefix);
    }
    // Bech32 builds a human-readable part from printable US-ASCII, 33 to 126.
    if let Some(actual) = prefix
        .chars()
        .find(|character| !matches!(u32::from(*character), 33..=126))
    {
        return Err(Error::RecoveryCodeInvalidPrefixCharacter { actual });
    }
    if prefix.len() > MAX_PREFIX_LEN {
        return Err(overlong(prefix, &text[separator + SEPARATOR.len_utf8()..]));
    }
    Ok(())
}

/// Answers a string whose prefix is longer than Bech32 lets one be — which is
/// what a code pasted twice is, the whole first copy standing before the second
/// copy's separator.
///
/// The `bech32` crate refuses such a prefix before it looks at the alphabet or
/// the checksum. KD-11 makes both of those first and compares the prefix with
/// `coffret` only after them, so they are made here in that order, giving the
/// answer the TypeScript reader gives. The prefix is named only when the
/// checksum verifies over it: short of that it may be a whole code, and a
/// refusal quoting it would print the Master Key in its other spelling.
fn overlong(prefix: &str, data: &str) -> Error {
    if let Some(actual) = data
        .chars()
        .find(|character| Fe32::from_char(*character).is_err())
    {
        return Error::RecoveryCodeInvalidCharacter { actual };
    }
    if data.len() < Bech32m::CHECKSUM_LENGTH {
        return Error::RecoveryCodeChecksumFailed;
    }
    // The prefix goes in as Bech32 expands it: every character's high three
    // bits, a zero, then every character's low five, all of the lowercase form.
    let lowered = || prefix.bytes().map(|byte| byte.to_ascii_lowercase());
    let expanded = lowered()
        .map(|byte| byte >> 5)
        .chain([0])
        .chain(lowered().map(|byte| byte & 0x1f))
        .map(|value| Fe32::try_from(value).expect("a value under 32 is a field element"));
    let elements = data
        .chars()
        .map(|character| Fe32::from_char(character).expect("the alphabet was checked above"));
    let mut engine = Engine::<Bech32m>::new();
    expanded
        .chain(elements)
        .for_each(|element| engine.input_fe(element));
    if *engine.residue() != Bech32m::TARGET_RESIDUE {
        return Error::RecoveryCodeChecksumFailed;
    }
    Error::UnknownRecoveryCodePrefix {
        actual: prefix.to_ascii_lowercase(),
    }
}

/// Names the check the string failed before its payload was ever reached.
///
/// [`divide`] has already made every check on the case, the separator and the
/// prefix, so what the `bech32` crate can still refuse is a character outside
/// the alphabet or a checksum. Anything else it names is answered as a checksum
/// failure, which never quotes the string.
fn rejected(error: CheckedHrpstringError) -> Error {
    match error {
        CheckedHrpstringError::Parse(UncheckedHrpstringError::Char(CharError::InvalidChar(
            actual,
        ))) => Error::RecoveryCodeInvalidCharacter { actual },
        // A checksum that does not verify, or too few characters after the
        // separator to hold one — a code cut short is answered the same way.
        _ => Error::RecoveryCodeChecksumFailed,
    }
}
