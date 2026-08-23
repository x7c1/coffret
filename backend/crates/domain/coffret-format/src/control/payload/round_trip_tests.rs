//! What survives a trip through a payload and back.

use ciborium::Value;

use super::decode::decode;
use super::encode::encode;
use super::testing::{body, epoch};
use super::{empty_map, to_bytes, ControlPayload};
use crate::padme;

// FM-13: every control-object payload carries `master_key_epoch`, and the
// epoch round-trips unchanged along with the kind's own fields.
#[test]
fn the_epoch_and_the_body_round_trip() {
    let payload = ControlPayload::new(epoch(3), body(&[("records", 2)]));
    let decoded = decode(&encode(&payload).expect("encoding succeeds")).expect("it round-trips");
    assert_eq!(decoded, payload);
}

// FM-11: what is encrypted is the map padded to its bucket, whatever the
// kind's own fields add up to — including sizes that cross a boundary.
#[test]
fn a_payload_is_encrypted_padded_to_its_bucket() {
    let mut grew_across_a_boundary = false;
    for field_count in 0..24 {
        let fields: Vec<(String, i64)> = (0..field_count)
            .map(|index| (format!("field_{index:03}"), i64::from(index)))
            .collect();
        let entries = fields
            .iter()
            .map(|(key, value)| (Value::Text(key.clone()), Value::from(*value)))
            .collect();
        let body = to_bytes(&Value::Map(entries)).expect("a map of integers serializes");

        let plaintext = encode(&ControlPayload::new(epoch(1), body)).expect("encoding succeeds");
        let mut padding = plaintext.as_slice();
        let _: Value = ciborium::from_reader(&mut padding).expect("the map is CBOR");
        let map_len = plaintext.len() - padding.len();

        assert_eq!(
            plaintext.len() as u64,
            padme::padded_len(map_len as u64),
            "a payload map of {map_len} bytes"
        );
        assert!(padding.iter().all(|byte| *byte == 0));
        grew_across_a_boundary |= plaintext.len() > map_len;

        // Padding is not something the reader has to be told about: the
        // payload that comes back is the one that went in.
        let decoded = decode(&plaintext).expect("a padded payload round-trips");
        assert_eq!(decoded.master_key_epoch, epoch(1));
    }
    assert!(
        grew_across_a_boundary,
        "no payload size in this test actually needed padding"
    );
}

#[test]
fn an_empty_payload_carries_only_the_epoch() {
    let payload = ControlPayload::empty(epoch(2));
    let decoded = decode(&encode(&payload).expect("encoding succeeds")).expect("it round-trips");
    assert_eq!(decoded, payload);
    assert_eq!(decoded.body, empty_map());
}
