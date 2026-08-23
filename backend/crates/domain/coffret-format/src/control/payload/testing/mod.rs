//! Helpers shared by the payload's tests.

use ciborium::Value;
use coffret_model::MasterKeyEpoch;

use super::encode::pad_to_bucket;
use super::to_bytes;

/// A kind's own fields, as the CBOR map a caller hands the framing.
pub(super) fn body(fields: &[(&str, i64)]) -> Vec<u8> {
    let entries = fields
        .iter()
        .map(|(key, value)| (Value::Text((*key).to_owned()), Value::from(*value)))
        .collect();
    to_bytes(&Value::Map(entries)).expect("a map of integers serializes")
}

pub(super) fn epoch(value: u64) -> MasterKeyEpoch {
    MasterKeyEpoch::new(value).expect("the epoch is valid")
}

/// A hand-built map as a decoder meets it: padded to its bucket (FM-11).
///
/// A test about what a map says is not a test about how it is padded, so the
/// maps written by hand here go through the same padding the encoder applies
/// and fail on the field they are about.
pub(super) fn padded(mut map: Vec<u8>) -> Vec<u8> {
    pad_to_bucket(&mut map).expect("padding a map this size succeeds");
    map
}
