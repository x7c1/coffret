use ciborium::Value;

use super::{as_map, malformed, to_bytes, ControlPayload, MASTER_KEY_EPOCH};
use crate::error::{Error, Result};
use crate::malformed_cbor::malformed_cbor;
use crate::padme;

/// Serializes a payload to the plaintext that gets encrypted: the kind's own
/// map with `master_key_epoch` added, then zero padding to its Padmé bucket.
///
/// A control object is one AEAD message, so its stored length is its payload's
/// length: unpadded, that length would count out for the provider whatever the
/// payload lists — the Entries an Index Snapshot names, the Containers a
/// Keyring maps. This is the meta section's rule (FM-9) applied to control
/// objects (FM-11).
pub(in crate::control) fn encode(payload: &ControlPayload) -> Result<Vec<u8>> {
    let mut entries = read_map(&payload.body)?;
    if entries
        .iter()
        .any(|(key, _)| key.as_text() == Some(MASTER_KEY_EPOCH))
    {
        return Err(malformed(format!(
            "the body already carries {MASTER_KEY_EPOCH}"
        )));
    }
    // The framing's own field goes first, so it is readable without walking the
    // kind's fields.
    entries.insert(
        0,
        (
            Value::Text(MASTER_KEY_EPOCH.to_owned()),
            Value::from(payload.master_key_epoch.get()),
        ),
    );
    let mut plaintext = to_bytes(&Value::Map(entries))?;
    pad_to_bucket(&mut plaintext)?;
    Ok(plaintext)
}

/// Grows a payload map to its Padmé bucket with zero bytes (FM-4, FM-11).
pub(super) fn pad_to_bucket(plaintext: &mut Vec<u8>) -> Result<()> {
    let padded = padme::padded_len(plaintext.len() as u64);
    let len = usize::try_from(padded).map_err(|_| Error::ControlPayloadTooLong { padded })?;
    plaintext.resize(len, 0);
    Ok(())
}

/// Reads one CBOR map, rejecting anything else and anything trailing it.
///
/// This is for the body the caller hands in, which is the kind's own map alone:
/// the padding is the framing's, and it is added once, around the whole payload.
fn read_map(bytes: &[u8]) -> Result<Vec<(Value, Value)>> {
    let mut remaining = bytes;
    let value: Value =
        ciborium::from_reader(&mut remaining).map_err(|error| malformed_cbor(error, malformed))?;
    if !remaining.is_empty() {
        return Err(malformed(format!(
            "{} bytes follow the payload map",
            remaining.len()
        )));
    }
    as_map(value)
}

#[cfg(test)]
mod tests {
    use super::super::testing::{body, epoch};
    use super::*;

    // FM-13: the epoch is a field of the payload map itself, not a wrapper
    // around it, so a reader of a future schema finds it beside the kind's own
    // fields.
    #[test]
    fn the_epoch_is_a_field_of_the_payload_map() {
        let bytes = encode(&ControlPayload::new(epoch(5), body(&[("records", 2)])))
            .expect("encoding succeeds");
        let value: Value = ciborium::from_reader(bytes.as_slice()).expect("the payload is CBOR");
        let entries = value.as_map().expect("the payload is a map");
        let keys: Vec<&str> = entries
            .iter()
            .map(|(key, _)| key.as_text().expect("keys are text"))
            .collect();
        assert_eq!(keys, ["master_key_epoch", "records"]);
        assert_eq!(
            entries[0].1.as_integer(),
            Some(ciborium::value::Integer::from(5u64))
        );
    }

    // The framing owns `master_key_epoch`; a body that also carries one would
    // leave two answers to which epoch wrote the object.
    #[test]
    fn a_body_that_claims_the_epoch_field_is_rejected() {
        let mut payload = ControlPayload::empty(epoch(1));
        payload.body = to_bytes(&Value::Map(vec![(
            Value::Text(MASTER_KEY_EPOCH.to_owned()),
            Value::from(9u64),
        )]))
        .expect("the map serializes");
        assert!(matches!(
            encode(&payload),
            Err(Error::MalformedControlPayload { .. })
        ));
    }

    // FM-4: Padmé leaves a length below the padding regime alone, so a payload
    // that short is stored as it is.
    #[test]
    fn a_map_below_the_padding_threshold_is_left_alone() {
        let mut short = vec![0xa0, 0x01, 0x02, 0x03, 0x04];
        pad_to_bucket(&mut short).expect("padding a short map succeeds");
        assert_eq!(short, [0xa0, 0x01, 0x02, 0x03, 0x04]);
    }

    // FM-4, FM-11: everything above that threshold grows to the next bucket
    // boundary with zeros, and a map already on one grows by nothing.
    #[test]
    fn a_map_grows_to_its_bucket_with_zeros() {
        for map_len in [8usize, 9, 100, 1_000] {
            let mut plaintext = vec![0x42; map_len];
            pad_to_bucket(&mut plaintext).expect("padding succeeds");

            let expected = padme::padded_len(map_len as u64) as usize;
            assert_eq!(plaintext.len(), expected, "a map of {map_len} bytes");
            assert!(
                plaintext[map_len..].iter().all(|byte| *byte == 0),
                "the padding of a {map_len}-byte map is not zero-filled"
            );
            assert!(
                plaintext[..map_len].iter().all(|byte| *byte == 0x42),
                "padding a {map_len}-byte map disturbed it"
            );
        }
    }
}
