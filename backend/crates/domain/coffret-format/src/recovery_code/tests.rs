use bech32::{Bech32m, ByteIterExt, Fe32, Fe32IterExt, Hrp};
use coffret_model::{MasterKey, MasterKeyEpoch, MAX_FORMAT_INTEGER};

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
    // The bound is the last epoch a Library can ever reach (spec: FM-19),
    // which is the value the eight epoch bytes have to carry back unchanged.
    for value in [1, MAX_FORMAT_INTEGER] {
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

// KD-11: epochs are numbered from 1 (spec: FM-13), so a code claiming
// epoch 0 carries no pair a Library could have written.
#[test]
fn epoch_zero_is_rejected() {
    let text = code_of(
        RecoveryCode::HUMAN_READABLE_PART,
        &payload(RecoveryCode::VERSION, 0, &master_key()),
    );

    let result = RecoveryCode::parse(&text);
    assert!(
        matches!(result, Err(Error::RecoveryCodeEpochOutOfRange { epoch: 0 })),
        "{result:?}"
    );
}

// KD-11, FM-19: the epoch bytes spell any `u64`, and the format admits only the
// numbers below 2^63, so a code carrying a larger one names no epoch either —
// the same refusal epoch 0 gets, in this layer's own vocabulary rather than the
// model's. The bound itself reads, which `round_trips_the_key_and_the_epoch`
// already shows.
#[test]
fn a_recovery_code_epoch_past_the_formats_integer_range_is_refused() {
    let past_the_bound = MAX_FORMAT_INTEGER + 1;
    let text = code_of(
        RecoveryCode::HUMAN_READABLE_PART,
        &payload(RecoveryCode::VERSION, past_the_bound, &master_key()),
    );

    let result = RecoveryCode::parse(&text);
    assert!(
        matches!(
            result,
            Err(Error::RecoveryCodeEpochOutOfRange { epoch }) if epoch == past_the_bound
        ),
        "{result:?}"
    );
}

// KD-11: a string with no `1` to divide at has lost its separator, whatever
// else it holds — including a whole prefix whose `o` the alphabet would refuse
// were it read as data.
#[test]
fn a_string_without_a_separator_is_refused_naming_the_separator() {
    for text in ["qqqqqqqq", "coffretqqqq"] {
        let result = RecoveryCode::parse(text);
        assert!(
            matches!(result, Err(Error::RecoveryCodeMissingSeparator)),
            "{text:?}: {result:?}"
        );
    }
}

// KD-11: a separator with nothing before it leaves no prefix to check.
#[test]
fn a_string_with_nothing_before_its_separator_is_refused_naming_the_prefix() {
    let result = RecoveryCode::parse("1qqqqqqq");
    assert!(
        matches!(result, Err(Error::RecoveryCodeEmptyPrefix)),
        "{result:?}"
    );
}

// KD-11: a prefix is printable US-ASCII, so a character outside it is named —
// before the data part is looked at, which is the order the TypeScript reader
// checks in too.
#[test]
fn a_prefix_holding_a_character_no_prefix_can_is_refused_naming_it() {
    let code = RecoveryCode::encode(master_key(), MasterKeyEpoch::FIRST);
    let text = code.as_str().replacen('o', "\u{f6}", 1);

    let result = RecoveryCode::parse(&text);
    assert!(
        matches!(
            result,
            Err(Error::RecoveryCodeInvalidPrefixCharacter { actual: '\u{f6}' })
        ),
        "{result:?}"
    );
}

// The refusals above are the ones a person reads, so each names what to change.
#[test]
fn each_refusal_of_the_division_names_its_check() {
    let cases = [
        ("coffretqqqq", "no separator"),
        ("1qqqqqqq", "nothing before its separator"),
        ("c\u{f6}ffret1qqqqqqqq", "prefix holds no character"),
    ];
    for (text, expected) in cases {
        let error = RecoveryCode::parse(text).expect_err("the string is refused");
        assert!(error.to_string().contains(expected), "{text:?}: {error}");
    }
}

// KD-11: a code pasted twice divides at the second copy's separator, leaving a
// prefix longer than Bech32 lets one be. The alphabet and the checksum come
// before the prefix, so the checksum ends the read — as it does in the
// TypeScript reader — and the refusal never quotes the first copy.
#[test]
fn a_code_pasted_twice_fails_the_checksum_without_quoting_itself() {
    let code = RecoveryCode::encode(master_key(), MasterKeyEpoch::FIRST);
    let text = format!("{}{}", code.as_str(), code.as_str());

    let error = RecoveryCode::parse(&text).expect_err("the string is refused");
    assert!(
        matches!(error, Error::RecoveryCodeChecksumFailed),
        "{error:?}"
    );
    assert!(!error.to_string().contains(&code.as_str()[8..]), "{error}");
}

// KD-11's order holds past the length Bech32 lets a prefix be: a character
// outside the alphabet is named before the checksum, and a string whose
// checksum verifies is refused for its prefix.
#[test]
fn a_prefix_past_bech32s_length_is_checked_in_kd_11s_order() {
    let prefix = "a".repeat(84);
    // Bech32m's checksum over 84 `a`s and eight `q`s, from the reference
    // algorithm, since the `bech32` crate writes no prefix this long.
    let verified = format!("{prefix}1qqqqqqqql5tuxw");
    let result = RecoveryCode::parse(&verified);
    assert!(
        matches!(&result, Err(Error::UnknownRecoveryCodePrefix { actual }) if *actual == prefix),
        "{result:?}"
    );

    let mistyped = format!("{prefix}1qqqqqqqql5tuxb");
    let result = RecoveryCode::parse(&mistyped);
    assert!(
        matches!(
            result,
            Err(Error::RecoveryCodeInvalidCharacter { actual: 'b' })
        ),
        "{result:?}"
    );

    let flipped = format!("{prefix}1qqqqqqqpl5tuxw");
    let result = RecoveryCode::parse(&flipped);
    assert!(
        matches!(result, Err(Error::RecoveryCodeChecksumFailed)),
        "{result:?}"
    );
}

// A code cut short after its separator still divides into a prefix and a data
// part, so it is the checksum that ends the read rather than a refusal of the
// division above — which is also what the TypeScript implementation answers
// with.
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

// The string is the key, so it reaches no diagnostic event through a derived
// formatter.
#[test]
fn debug_does_not_leak_the_code() {
    let code = RecoveryCode::encode(master_key(), MasterKeyEpoch::FIRST);
    assert_eq!(format!("{code:?}"), "RecoveryCode(<redacted>)");
    assert_eq!(format!("{code}"), code.as_str());
}
