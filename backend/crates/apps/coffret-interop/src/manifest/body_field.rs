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

    /// A field carrying a signed integer.
    ///
    /// A value at or above zero is stated as an unsigned one, so that one
    /// number has one spelling in the manifest whichever side wrote it: an
    /// `mtime` of 0 read back off the wire is not distinguishable from an
    /// unsigned 0, and two spellings would make the comparison depend on which
    /// implementation produced the set.
    pub fn int(key: &str, value: i64) -> Self {
        match u64::try_from(value) {
            Ok(value) => Self::uint(key, value),
            Err(_) => Self {
                key: key.to_owned(),
                value: BodyValue::Int { value },
            },
        }
    }

    /// A field carrying an array.
    pub fn array(key: &str, value: Vec<BodyValue>) -> Self {
        Self {
            key: key.to_owned(),
            value: BodyValue::Array { value },
        }
    }

    /// A field carrying a nested map.
    pub fn map(key: &str, value: Vec<BodyField>) -> Self {
        Self {
            key: key.to_owned(),
            value: BodyValue::Map { value },
        }
    }

    /// This field's value as the CBOR item a payload map carries.
    pub(super) fn to_cbor(&self) -> Result<Value> {
        self.value
            .to_cbor()
            .with_context(|| format!("field {:?}", self.key))
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

    // The nested spellings the payload schemas need (FM-15, FM-16), which both
    // implementations parse and render.
    #[test]
    fn the_nested_spellings_are_the_ones_both_implementations_write() {
        let field = BodyField::array(
            "additions",
            vec![BodyValue::Map {
                value: vec![BodyField::int("mtime", -1)],
            }],
        );
        let json = serde_json::to_string(&field).expect("it serializes");
        assert_eq!(
            json,
            r#"{"key":"additions","type":"array","value":[{"type":"map","value":[{"key":"mtime","type":"int","value":-1}]}]}"#
        );
    }

    #[test]
    fn a_non_negative_signed_value_is_stated_as_an_unsigned_one() {
        assert_eq!(BodyField::int("mtime", 7), BodyField::uint("mtime", 7));
    }
}
