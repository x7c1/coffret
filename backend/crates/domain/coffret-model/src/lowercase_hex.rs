/// Whether `value` is a non-empty run of lowercase hex digits.
///
/// Every hex spelling in coffret is lowercase, because two spellings of one
/// value would be two names for one thing — a Container's object name (FM-3),
/// or a Keyring's `set_digest` both in a replica name and in the commitment a
/// Journal record selects that replica set with (FM-12, KL-3).
pub(crate) fn is_nonempty_lowercase_hex(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}
