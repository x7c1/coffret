//! Printing one control object's payload with the field names the
//! field-number registry gives its field numbers.
//!
//! A stored payload carries its fields under field numbers (spec: FM-9,
//! FM-15, FM-16, FM-17), which makes a raw dump a list of numbers to look up by
//! hand. This opens an object fetched from a Library — with the Master Key its
//! Recovery Code carries — and prints the payload as JSON keyed by name, for a
//! person checking what a device actually wrote.

use std::path::Path;

use anyhow::{bail, Context, Result};
use ciborium::Value;
use coffret_format::field_numbers::{named_view, payload_map};
use coffret_format::{decode_control_object, Purpose, PurposeKey, RecoveryCode};
use coffret_model::ControlObjectKind;
use serde_json::{json, Map};

use crate::hex;

/// Opens the control object at `object`, stored under `object_name`, and
/// returns its payload as JSON keyed by field name.
///
/// The kind rides in the authenticated header, which only the kind's own key
/// opens, so each kind's key is tried in turn and the one that opens the object
/// says which kind it is.
pub fn dump(object: &Path, object_name: &str, recovery_code: &Path) -> Result<String> {
    let bytes = std::fs::read(object).context("reading the control object")?;
    let code = std::fs::read_to_string(recovery_code).context("reading the Recovery Code")?;
    let code = RecoveryCode::parse(&code).context("parsing the Recovery Code")?;

    let opened = ControlObjectKind::ALL
        .iter()
        .find_map(|kind| {
            let key = PurposeKey::derive(code.master_key(), Purpose::of_control_object(*kind));
            decode_control_object(&bytes, object_name, &key).ok()
        })
        .context("no control-object kind's key opens this object under this name")?;

    let body: Value = ciborium::from_reader(opened.payload.body.as_slice())
        .context("the payload body is not readable CBOR")?;
    let kind = payload_map(opened.kind);
    let rendered = json!({
        "kind": format!("{:?}", opened.kind),
        "map": kind.map(),
        "generation": opened.generation.get(),
        "master_key_epoch": opened.payload.master_key_epoch.get(),
        "body": to_json(&named_view(body, kind))?,
    });
    serde_json::to_string_pretty(&rendered).context("rendering the payload")
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
                    other => bail!("a payload map key is {other:?}, which this dump cannot name"),
                };
                map.insert(key, to_json(value)?);
            }
            serde_json::Value::Object(map)
        }
        other => bail!("a payload carries {other:?}, which this dump cannot render"),
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
