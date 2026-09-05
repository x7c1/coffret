use ciborium::Value;
use coffret_model::MasterKeyEpoch;

use super::{as_map, to_bytes, ControlPayload, MASTER_KEY_EPOCH};
use crate::control::cbor::as_bounded_uint;
use crate::error::{Error, Result};
use crate::padme;

/// Parses a payload plaintext, insisting that it says which epoch encrypted it.
pub(in crate::control) fn decode(plaintext: &[u8]) -> Result<ControlPayload> {
    let mut entries = read_padded_map(plaintext)?;
    let position = entries
        .iter()
        .position(|(key, _)| key.as_text() == Some(MASTER_KEY_EPOCH))
        .ok_or(Error::MissingMasterKeyEpoch)?;
    let (_, epoch) = entries.remove(position);
    // The epoch is read before the body, so it does not pass through `Fields`;
    // it is held to the same bound all the same (FM-19), by the same reading.
    let epoch = as_bounded_uint(&epoch).ok_or_else(|| Error::MalformedControlPayload {
        detail: format!("{MASTER_KEY_EPOCH} is not an unsigned integer below 2^63"),
    })?;

    // The two carriers that spell an epoch as 8 raw bytes — a Recovery Code
    // (KD-11) and a stored Master Key (KD-9) — name their own refusal, since
    // nothing has stated the bound by the time they read those bytes. Here the
    // bound has just been stated, so all the model is left to refuse for is
    // epoch 0 and its refusal names exactly that: passing it through is the one
    // spelling of that rule rather than a second.
    Ok(ControlPayload::new(
        MasterKeyEpoch::new(epoch)?,
        to_bytes(&Value::Map(entries))?,
    ))
}

/// Reads the map a payload plaintext carries, holding what follows it to
/// FM-11's padding rule.
///
/// CBOR is self-delimiting, so nothing records where the map ends: the map is
/// read first and the padding is whatever is left. That tail has to be exactly
/// the zero bytes that carry the map to its Padmé bucket — a plaintext of any
/// other length was written by something that did not pad as the rule says, and
/// a non-zero byte would make the padding a place to ride bytes past a reader.
fn read_padded_map(plaintext: &[u8]) -> Result<Vec<(Value, Value)>> {
    let mut padding = plaintext;
    let value: Value =
        ciborium::from_reader(&mut padding).map_err(|error| Error::MalformedControlPayload {
            detail: error.to_string(),
        })?;

    let map_len = (plaintext.len() - padding.len()) as u64;
    let expected = padme::padded_len(map_len);
    if expected != plaintext.len() as u64 {
        return Err(Error::ControlPaddingLengthMismatch {
            expected,
            actual: plaintext.len() as u64,
        });
    }
    if padding.iter().any(|byte| *byte != 0) {
        return Err(Error::NonZeroControlPadding);
    }
    as_map(value)
}

#[cfg(test)]
mod tests {
    use super::super::encode::encode;
    use super::super::testing::{body, epoch, padded};
    use super::*;

    // FM-13: a payload that does not say which epoch encrypted it is rejected.
    #[test]
    fn a_payload_without_the_epoch_is_rejected() {
        let result = decode(&padded(body(&[("records", 2)])));
        assert!(
            matches!(result, Err(Error::MissingMasterKeyEpoch)),
            "expected a payload without an epoch to be rejected, got {result:?}"
        );
    }

    #[test]
    fn a_payload_that_is_not_a_map_is_rejected() {
        let bytes = to_bytes(&Value::Text("not a map".to_owned())).expect("text serializes");
        let result = decode(&padded(bytes));
        assert!(
            matches!(result, Err(Error::ControlPayloadNotAMap)),
            "expected a payload that is not a map to be rejected, got {result:?}"
        );
    }

    // FM-13: epoch numbering starts at 1, so a payload claiming epoch 0 is not
    // one this build can trust to name a Master Key.
    #[test]
    fn an_epoch_below_one_is_rejected() {
        let bytes = to_bytes(&Value::Map(vec![(
            Value::Text(MASTER_KEY_EPOCH.to_owned()),
            Value::from(0u64),
        )]))
        .expect("the map serializes");
        let result = decode(&padded(bytes));
        assert!(
            matches!(
                result,
                Err(Error::Model(coffret_model::Error::EpochOutOfRange {
                    epoch: 0
                }))
            ),
            "expected epoch 0 to be rejected, got {result:?}"
        );
    }

    #[test]
    fn a_non_integer_epoch_is_rejected() {
        let bytes = to_bytes(&Value::Map(vec![(
            Value::Text(MASTER_KEY_EPOCH.to_owned()),
            Value::Text("first".to_owned()),
        )]))
        .expect("the map serializes");
        assert!(matches!(
            decode(&padded(bytes)),
            Err(Error::MalformedControlPayload { .. })
        ));
    }

    // FM-11: the plaintext is the map and its padding and nothing else, so a
    // zero byte beyond the bucket is a length no writer following the rule
    // produces.
    #[test]
    fn a_plaintext_longer_than_the_bucket_is_rejected() {
        let mut bytes = encode(&ControlPayload::empty(epoch(1))).expect("encoding succeeds");
        let padded = bytes.len() as u64;
        bytes.push(0x00);
        assert!(
            matches!(
                decode(&bytes),
                Err(Error::ControlPaddingLengthMismatch { expected, actual })
                    if expected == padded && actual == padded + 1
            ),
            "expected a plaintext past the bucket to be rejected"
        );
    }

    // FM-11: the padding is not a place to ride bytes past a reader, so every
    // byte of it is checked.
    #[test]
    fn a_non_zero_byte_in_the_padding_is_rejected() {
        let plaintext = encode(&ControlPayload::empty(epoch(1))).expect("encoding succeeds");
        let mut padding = plaintext.as_slice();
        let _: Value = ciborium::from_reader(&mut padding).expect("the map is CBOR");
        let map_len = plaintext.len() - padding.len();
        assert!(map_len < plaintext.len(), "this payload carries no padding");

        for index in map_len..plaintext.len() {
            let mut tampered = plaintext.clone();
            tampered[index] = 0x01;
            let result = decode(&tampered);
            assert!(
                matches!(result, Err(Error::NonZeroControlPadding)),
                "byte {index} of the padding was not checked, got {result:?}"
            );
        }
    }

    // FM-11: a writer that skipped the padding leaks the size the padding
    // exists to blur, so its object is refused rather than quietly read.
    #[test]
    fn an_unpadded_payload_is_rejected() {
        let plaintext = encode(&ControlPayload::empty(epoch(1))).expect("encoding succeeds");
        let mut padding = plaintext.as_slice();
        let _: Value = ciborium::from_reader(&mut padding).expect("the map is CBOR");
        let map_len = plaintext.len() - padding.len();

        let result = decode(&plaintext[..map_len]);
        assert!(
            matches!(
                result,
                Err(Error::ControlPaddingLengthMismatch { expected, actual })
                    if expected == plaintext.len() as u64 && actual == map_len as u64
            ),
            "expected an unpadded payload to be rejected, got {result:?}"
        );
    }
}
