use ciborium::Value;
use coffret_model::MasterKeyEpoch;

use crate::error::{Error, Result};

mod decode;
pub(super) use decode::decode;

mod encode;
pub(super) use encode::encode;

#[cfg(test)]
mod round_trip_tests;

#[cfg(test)]
mod testing;

/// What a control object carries inside its AEAD message.
///
/// The payload is one CBOR map, encrypted as that map followed by zero padding
/// up to its Padmé bucket (FM-11). This module owns exactly one of its fields —
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

fn as_map(value: Value) -> Result<Vec<(Value, Value)>> {
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
