use bech32::{Bech32m, ByteIterExt, Fe32, Fe32IterExt, Hrp};
use coffret_model::{MasterKey, MasterKeyEpoch};

use super::{RecoveryCode, HRP};
use crate::error::Error;

/// A key whose every byte differs, so a reader that dropped or reordered bytes
/// lands somewhere else rather than on the same value.
fn master_key() -> MasterKey {
    let mut bytes = [0u8; MasterKey::BYTE_LEN];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = (index as u8).wrapping_mul(31).wrapping_add(7);
    }
    MasterKey::from_bytes(bytes)
}

fn epoch(value: u64) -> MasterKeyEpoch {
    MasterKeyEpoch::new(value).expect("the value numbers an epoch")
}

/// Writes a code from a payload of any length, under any prefix — which is how
/// the rejections below are built, since [`RecoveryCode::encode`] can only
/// produce well-formed ones.
fn code_of(hrp: &str, payload: &[u8]) -> String {
    bech32::encode_lower::<Bech32m>(
        Hrp::parse(hrp).expect("the prefix is a human-readable part"),
        payload,
    )
    .expect("the payload is inside Bech32's length limit")
}

/// The payload KD-11 defines, as the rejection cases start from before editing.
fn payload(version: u8, epoch: u64, key: &MasterKey) -> Vec<u8> {
    let mut payload = vec![version];
    payload.extend_from_slice(&epoch.to_be_bytes());
    payload.extend_from_slice(key.as_bytes());
    payload
}

// KD-11: a code carries the Master Key and the epoch, and reading one back
// gives exactly the pair that was written.
#[test]
fn round_trips_the_key_and_the_epoch() {
    for value in [1, u64::MAX] {
        let code = RecoveryCode::encode(master_key(), epoch(value));
        let parsed = RecoveryCode::parse(code.as_str()).expect("the code this crate wrote parses");

        assert_eq!(parsed.master_key().as_bytes(), master_key().as_bytes());
        assert_eq!(parsed.epoch(), epoch(value));
        assert_eq!(parsed.as_str(), code.as_str());
    }
}

// KD-11: `coffret1`, 66 data characters and a 6-character checksum, lowercase.
#[test]
fn is_eighty_lowercase_characters_under_the_coffret_prefix() {
    let code = RecoveryCode::encode(master_key(), MasterKeyEpoch::FIRST);
    let text = code.as_str();

    assert_eq!(text.len(), RecoveryCode::TEXT_LEN);
    assert_eq!(text.len(), 80);
    assert_eq!(text, text.to_lowercase());
    assert!(text.starts_with("coffret1"), "{text}");
}

// KD-11: the printed grouping is presentation, so it reads back as the same
// code — and it is everything after `coffret1` that is grouped, not the prefix.
#[test]
fn the_grouped_printing_form_parses_back() {
    let code = RecoveryCode::encode(master_key(), epoch(42));
    let grouped = code.to_grouped_string();

    let (prefix, data) = grouped
        .split_once(' ')
        .expect("the prefix stands apart from the groups");
    assert_eq!(prefix, "coffret1");
    // The checksum is grouped along with the data characters, so the 72
    // characters after `coffret1` are 18 full groups and no short one.
    let groups: Vec<&str> = data.split(' ').collect();
    assert_eq!(groups.len(), 18, "{grouped:?}");
    for group in groups {
        assert_eq!(
            group.len(),
            RecoveryCode::GROUP_LEN,
            "{group:?} in {grouped:?}"
        );
    }

    let parsed = RecoveryCode::parse(&grouped).expect("the grouped form is the same code");
    assert_eq!(parsed.as_str(), code.as_str());
    assert_eq!(parsed.epoch(), epoch(42));
}

// KD-11: whitespace and hyphens go before anything else, so however the user
// broke the string up on paper, the code is the code.
#[test]
fn whitespace_and_hyphens_are_stripped() {
    let code = RecoveryCode::encode(master_key(), epoch(9));
    let text = code.as_str();
    let broken = format!("  {}-{}\n\t{}  ", &text[..20], &text[20..50], &text[50..]);

    let parsed = RecoveryCode::parse(&broken).expect("the strippable characters are stripped");
    assert_eq!(parsed.as_str(), text);
}

// KD-11: Bech32 admits a code written entirely in either case; the canonical
// spelling this crate hands back is the lowercase one.
#[test]
fn an_uppercase_copy_parses() {
    let code = RecoveryCode::encode(master_key(), epoch(2));
    let parsed =
        RecoveryCode::parse(&code.as_str().to_uppercase()).expect("an uppercase copy is the code");

    assert_eq!(parsed.as_str(), code.as_str());
    assert_eq!(parsed.master_key().as_bytes(), master_key().as_bytes());
}

// KD-11: a mixture of cases is not a third spelling of the code — no checksum
// can be verified over it.
#[test]
fn a_mixed_case_copy_is_rejected() {
    let code = RecoveryCode::encode(master_key(), MasterKeyEpoch::FIRST);
    let mixed = format!(
        "{}{}",
        code.as_str()[..40].to_uppercase(),
        &code.as_str()[40..]
    );

    let result = RecoveryCode::parse(&mixed);
    assert!(
        matches!(result, Err(Error::RecoveryCodeMixedCase)),
        "{result:?}"
    );
}

// KD-11: the checksum is what makes a hand copy safe — one wrong character
// ends the read rather than yielding a different Master Key.
#[test]
fn a_flipped_character_fails_the_checksum() {
    let code = RecoveryCode::encode(master_key(), MasterKeyEpoch::FIRST);
    let text = code.as_str();
    let flipped_at = 30;
    let original = &text[flipped_at..flipped_at + 1];
    let replacement = if original == "q" { "p" } else { "q" };
    let flipped = format!(
        "{}{replacement}{}",
        &text[..flipped_at],
        &text[flipped_at + 1..]
    );

    let result = RecoveryCode::parse(&flipped);
    assert!(
        matches!(result, Err(Error::RecoveryCodeChecksumFailed)),
        "{result:?}"
    );
}

// KD-11: a character outside the Bech32 alphabet is a transcription mistake,
// and the four the alphabet leaves out are the ones people make.
#[test]
fn a_character_outside_the_alphabet_is_rejected() {
    let code = RecoveryCode::encode(master_key(), MasterKeyEpoch::FIRST);
    let typo = format!("{}b{}", &code.as_str()[..30], &code.as_str()[31..]);

    let result = RecoveryCode::parse(&typo);
    assert!(
        matches!(
            result,
            Err(Error::RecoveryCodeInvalidCharacter { actual: 'b' })
        ),
        "{result:?}"
    );
}

// KD-11: a well-formed Bech32m string under someone else's prefix is not a
// Recovery Code, however sound its checksum.
#[test]
fn another_prefix_is_rejected() {
    let text = code_of("wrong", &payload(RecoveryCode::VERSION, 1, &master_key()));

    let result = RecoveryCode::parse(&text);
    assert!(
        matches!(result, Err(Error::UnknownRecoveryCodePrefix { ref actual }) if actual == "wrong"),
        "{result:?}"
    );
}

// KD-11: the payload is 41 bytes exactly — 66 data characters — so a code
// carrying one byte fewer or more is refused rather than read part-way.
#[test]
fn a_payload_of_another_length_is_rejected() {
    for length in [40, 42] {
        let text = code_of(RecoveryCode::HUMAN_READABLE_PART, &vec![0x11; length]);

        let result = RecoveryCode::parse(&text);
        assert!(
            matches!(result, Err(Error::RecoveryCodeLengthMismatch { actual }) if actual != RecoveryCode::DATA_LEN),
            "{length} bytes: {result:?}"
        );
    }
}

// KD-11: the two bits left over past the 41st byte are zero, so a writer that
// put anything there wrote a string this form does not define.
#[test]
fn non_zero_padding_bits_are_rejected() {
    let payload = payload(RecoveryCode::VERSION, 1, &master_key());
    let mut characters: Vec<Fe32> = payload.iter().copied().bytes_to_fes().collect();
    assert_eq!(characters.len(), RecoveryCode::DATA_LEN);
    let last = characters.last_mut().expect("the data part is not empty");
    *last = Fe32::try_from(last.to_u8() | 0b11).expect("the value is a field element");

    let text: String = characters
        .into_iter()
        .with_checksum::<Bech32m>(&HRP)
        .chars()
        .collect();

    let result = RecoveryCode::parse(&text);
    assert!(
        matches!(result, Err(Error::NonZeroRecoveryCodePadding)),
        "{result:?}"
    );
}

// KD-11: the version byte leads the payload so a later form can change what
// follows it; a build that does not know a version reads none of it.
#[test]
fn an_unknown_version_is_rejected() {
    let text = code_of(
        RecoveryCode::HUMAN_READABLE_PART,
        &payload(0x02, 1, &master_key()),
    );

    let result = RecoveryCode::parse(&text);
    assert!(
        matches!(
            result,
            Err(Error::UnsupportedRecoveryCodeVersion { actual: 0x02 })
        ),
        "{result:?}"
    );
}

// KD-11: epochs are numbered from 1 (FM-13), so a code claiming epoch 0
// carries no pair a Library could have written.
#[test]
fn epoch_zero_is_rejected() {
    let text = code_of(
        RecoveryCode::HUMAN_READABLE_PART,
        &payload(RecoveryCode::VERSION, 0, &master_key()),
    );

    let result = RecoveryCode::parse(&text);
    assert!(
        matches!(
            result,
            Err(Error::Model(coffret_model::Error::EpochOutOfRange))
        ),
        "{result:?}"
    );
}

// KD-11: a string that divides into no prefix and data part is not a code with
// something wrong in it — there is nothing to run any of the other checks over.
#[test]
fn a_string_that_divides_into_no_prefix_and_data_part_is_rejected() {
    for text in ["qqqqqqqq", "1qqqqqqq"] {
        let result = RecoveryCode::parse(text);
        assert!(
            matches!(result, Err(Error::MalformedRecoveryCode)),
            "{text:?}: {result:?}"
        );
    }
}

// A code cut short after its separator still divides into a prefix and a data
// part, so it is the checksum that ends the read rather than the refusal above
// — which is also what the TypeScript implementation answers with.
#[test]
fn a_code_cut_short_after_the_separator_fails_the_checksum() {
    for text in ["coffret1", "coffret1qqq"] {
        let result = RecoveryCode::parse(text);
        assert!(
            matches!(result, Err(Error::RecoveryCodeChecksumFailed)),
            "{text:?}: {result:?}"
        );
    }
}

// The string is the key, so it reaches no log line through a derived formatter.
#[test]
fn debug_does_not_leak_the_code() {
    let code = RecoveryCode::encode(master_key(), MasterKeyEpoch::FIRST);
    assert_eq!(format!("{code:?}"), "RecoveryCode(<redacted>)");
    assert_eq!(format!("{code}"), code.as_str());
}
