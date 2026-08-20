use ciborium::Value;
use coffret_model::MasterKeyEpoch;

use crate::error::{Error, Result};

/// What a control object carries inside its AEAD message.
///
/// The payload is one CBOR map. This module owns exactly one of its fields —
/// `master_key_epoch`, which every control object carries whatever its kind — and
/// treats the rest as the kind's own business: the caller hands over the CBOR map
/// of its fields, and gets that map back on the way out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlPayload {
    /// The Master Key epoch that encrypted this object.
    pub master_key_epoch: MasterKeyEpoch,
    /// The kind's own fields, as the CBOR map they were serialized to.
    pub body: Vec<u8>,
}

/// The one payload field the framing itself defines.
const MASTER_KEY_EPOCH: &str = "master_key_epoch";

impl ControlPayload {
    /// A payload of `body` — a CBOR map — written under `master_key_epoch`.
    pub fn new(master_key_epoch: MasterKeyEpoch, body: Vec<u8>) -> Self {
        Self {
            master_key_epoch,
            body,
        }
    }

    /// A payload carrying nothing but the epoch, for a kind with no fields yet.
    pub fn empty(master_key_epoch: MasterKeyEpoch) -> Self {
        Self::new(master_key_epoch, empty_map())
    }
}

/// The CBOR spelling of a map with no entries.
fn empty_map() -> Vec<u8> {
    to_bytes(&Value::Map(Vec::new())).expect("an empty map always serializes")
}

/// Serializes a payload: the kind's own map with `master_key_epoch` added.
pub(super) fn encode(payload: &ControlPayload) -> Result<Vec<u8>> {
    let mut entries = read_map(&payload.body)?;
    if entries
        .iter()
        .any(|(key, _)| key.as_text() == Some(MASTER_KEY_EPOCH))
    {
        return Err(Error::MalformedControlPayload {
            detail: format!("the body already carries {MASTER_KEY_EPOCH}"),
        });
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
    to_bytes(&Value::Map(entries))
}

/// Parses a payload, insisting that it says which epoch encrypted it.
pub(super) fn decode(bytes: &[u8]) -> Result<ControlPayload> {
    let mut entries = read_map(bytes)?;
    let position = entries
        .iter()
        .position(|(key, _)| key.as_text() == Some(MASTER_KEY_EPOCH))
        .ok_or(Error::MissingMasterKeyEpoch)?;
    let (_, epoch) = entries.remove(position);
    let epoch = epoch
        .as_integer()
        .and_then(|integer| u64::try_from(integer).ok())
        .ok_or_else(|| Error::MalformedControlPayload {
            detail: format!("{MASTER_KEY_EPOCH} is not an unsigned integer"),
        })?;

    Ok(ControlPayload::new(
        MasterKeyEpoch::new(epoch)?,
        to_bytes(&Value::Map(entries))?,
    ))
}

/// Reads one CBOR map, rejecting anything else and anything trailing it.
fn read_map(bytes: &[u8]) -> Result<Vec<(Value, Value)>> {
    let mut remaining = bytes;
    let value: Value =
        ciborium::from_reader(&mut remaining).map_err(|error| Error::MalformedControlPayload {
            detail: error.to_string(),
        })?;
    if !remaining.is_empty() {
        return Err(Error::MalformedControlPayload {
            detail: format!("{} bytes follow the payload map", remaining.len()),
        });
    }
    match value {
        Value::Map(entries) => Ok(entries),
        _ => Err(Error::ControlPayloadNotAMap),
    }
}

fn to_bytes(value: &Value) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    ciborium::into_writer(value, &mut bytes).map_err(|error| {
        Error::ControlPayloadEncodeFailed {
            detail: error.to_string(),
        }
    })?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(fields: &[(&str, i64)]) -> Vec<u8> {
        let entries = fields
            .iter()
            .map(|(key, value)| (Value::Text((*key).to_owned()), Value::from(*value)))
            .collect();
        to_bytes(&Value::Map(entries)).expect("a map of integers serializes")
    }

    fn epoch(value: u64) -> MasterKeyEpoch {
        MasterKeyEpoch::new(value).expect("the epoch is valid")
    }

    // FM-13: every control-object payload carries `master_key_epoch`, and the
    // epoch round-trips unchanged along with the kind's own fields.
    #[test]
    fn the_epoch_and_the_body_round_trip() {
        let payload = ControlPayload::new(epoch(3), body(&[("records", 2)]));
        let decoded =
            decode(&encode(&payload).expect("encoding succeeds")).expect("it round-trips");
        assert_eq!(decoded, payload);
    }

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

    // FM-13: a payload that does not say which epoch encrypted it is rejected.
    #[test]
    fn a_payload_without_the_epoch_is_rejected() {
        let result = decode(&body(&[("records", 2)]));
        assert!(
            matches!(result, Err(Error::MissingMasterKeyEpoch)),
            "expected a payload without an epoch to be rejected, got {result:?}"
        );
    }

    #[test]
    fn a_payload_that_is_not_a_map_is_rejected() {
        let bytes = to_bytes(&Value::Text("not a map".to_owned())).expect("text serializes");
        let result = decode(&bytes);
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
        let result = decode(&bytes);
        assert!(
            matches!(
                result,
                Err(Error::Model(coffret_model::Error::EpochOutOfRange))
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
            decode(&bytes),
            Err(Error::MalformedControlPayload { .. })
        ));
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

    #[test]
    fn trailing_bytes_after_the_map_are_rejected() {
        let mut bytes = encode(&ControlPayload::empty(epoch(1))).expect("encoding succeeds");
        bytes.push(0x00);
        assert!(matches!(
            decode(&bytes),
            Err(Error::MalformedControlPayload { .. })
        ));
    }

    #[test]
    fn an_empty_payload_carries_only_the_epoch() {
        let payload = ControlPayload::empty(epoch(2));
        let decoded =
            decode(&encode(&payload).expect("encoding succeeds")).expect("it round-trips");
        assert_eq!(decoded, payload);
        assert_eq!(decoded.body, empty_map());
    }
}
