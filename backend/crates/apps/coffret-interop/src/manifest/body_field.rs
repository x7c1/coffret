use anyhow::{Context, Result};
use ciborium::Value;
use serde::{Deserialize, Serialize};

use crate::hex;

use super::BodyValue;

/// One field of a control object's payload body, as the manifest states it.
///
/// A body is the kind's own CBOR map, which the framing treats as opaque. The
/// manifest therefore describes it field by field in a small typed vocabulary
/// rather than as bytes: the two implementations legitimately serialize a map
/// differently, so only the decoded fields can be compared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BodyField {
    /// The map key, which is always text.
    pub key: String,
    /// The value the key carries.
    #[serde(flatten)]
    pub value: BodyValue,
}

impl BodyField {
    /// A field carrying an unsigned integer.
    pub fn uint(key: &str, value: u64) -> Self {
        Self {
            key: key.to_owned(),
            value: BodyValue::Uint { value },
        }
    }

    /// A field carrying text.
    pub fn text(key: &str, value: &str) -> Self {
        Self {
            key: key.to_owned(),
            value: BodyValue::Text {
                value: value.to_owned(),
            },
        }
    }

    /// A field carrying a byte string.
    pub fn bytes(key: &str, value: &[u8]) -> Self {
        Self {
            key: key.to_owned(),
            value: BodyValue::Bytes {
                value: hex::encode(value),
            },
        }
    }

    /// This field's value as the CBOR item a payload map carries.
    pub(super) fn to_cbor(&self) -> Result<Value> {
        Ok(match &self.value {
            BodyValue::Uint { value } => Value::from(*value),
            BodyValue::Text { value } => Value::Text(value.clone()),
            BodyValue::Bytes { value } => {
                Value::Bytes(hex::decode(value).with_context(|| format!("field {:?}", self.key))?)
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_json_spelling_is_the_one_both_implementations_write() {
        let json = serde_json::to_string(&BodyField::uint("records", 2)).expect("it serializes");
        assert_eq!(json, r#"{"key":"records","type":"uint","value":2}"#);
    }
}
