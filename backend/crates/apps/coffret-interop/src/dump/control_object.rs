//! A control object's payload, keyed by the names the field-number registry
//! gives its field numbers.

use ciborium::Value;
use coffret_format::field_numbers::{named_view, payload_map};
use coffret_format::DecodedControlObject;
use serde_json::{json, Map};

use crate::{hex, Error, Result};

/// A control object's payload, keyed by field name.
pub(super) fn control_object(opened: &DecodedControlObject) -> Result<serde_json::Value> {
    let body: Value = ciborium::from_reader(opened.payload.body.as_slice())
        .map_err(|cause| Error::BodyUnreadable { cause })?;
    let kind = payload_map(opened.kind);
    Ok(json!({
        "kind": format!("{:?}", opened.kind),
        "map": kind.map(),
        "generation": opened.generation.get(),
        "master_key_epoch": opened.payload.master_key_epoch.get(),
        "body": to_json(&named_view(body, kind))?,
    }))
}

/// One named CBOR value as JSON: byte strings as lowercase hex, and a map key
/// the registry does not name kept as it was written (an integer as its digits,
/// a text key already quoted by [`named_view`]).
fn to_json(value: &Value) -> Result<serde_json::Value> {
    Ok(match value {
        Value::Integer(integer) => {
            let integer = i128::from(*integer);
            match i64::try_from(integer) {
                Ok(small) => json!(small),
                Err(_) => json!(integer.to_string()),
            }
        }
        Value::Bytes(bytes) => json!(hex::encode(bytes)),
        Value::Text(text) => json!(text),
        Value::Bool(flag) => json!(flag),
        Value::Null => serde_json::Value::Null,
        Value::Float(float) => json!(float),
        Value::Array(items) => {
            serde_json::Value::Array(items.iter().map(to_json).collect::<Result<Vec<_>>>()?)
        }
        Value::Map(entries) => {
            let mut map = Map::new();
            for (key, value) in entries {
                let key = match key {
                    Value::Text(text) => text.clone(),
                    Value::Integer(integer) => i128::from(*integer).to_string(),
                    other => {
                        return Err(Error::UnnamableKey {
                            found: other.clone(),
                        })
                    }
                };
                map.insert(key, to_json(value)?);
            }
            serde_json::Value::Object(map)
        }
        other => {
            return Err(Error::UnrenderableValue {
                found: other.clone(),
            })
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // The point of the dump: a Keyring element's field numbers come out as the
    // names the specification uses, and an unknown field number stays visible.
    #[test]
    fn a_body_is_rendered_by_name() {
        let body = Value::Map(vec![
            (Value::from(1u64), Value::from(1u64)),
            (
                Value::from(2u64),
                Value::Array(vec![Value::Map(vec![
                    (Value::from(0u64), Value::Bytes(vec![0xab, 0x01])),
                    (Value::from(2u64), Value::Bool(true)),
                ])]),
            ),
            (Value::from(40u64), Value::from(7u64)),
        ]);
        let rendered = to_json(&named_view(body, &coffret_format::field_numbers::KEYRING))
            .expect("the body renders");
        assert_eq!(
            rendered,
            json!({
                "schema": 1,
                "mapping": [{ "id": "ab01", "key_lost": true }],
                "40": 7,
            })
        );
    }
}
