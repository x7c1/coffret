use anyhow::Result;
use ciborium::Value;
use serde::{Deserialize, Serialize};

use crate::hex;

use super::BodyField;

/// The value types a manifest may state for a body field.
///
/// The vocabulary is small on purpose — the manifest describes a payload body
/// field by field rather than as bytes, because the two implementations
/// legitimately order and spell a CBOR map differently and only the decoded
/// fields are normative. [`Array`](Self::Array) and [`Map`](Self::Map) are what
/// let it describe the payloads whose fields are not flat: a Journal record's
/// additions each carry an entry table (FM-15), and an Index Snapshot's
/// Containers and Entries are arrays of maps (FM-16).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BodyValue {
    /// An unsigned integer.
    Uint {
        /// The number itself.
        value: u64,
    },
    /// A signed integer, for the one field that may be negative: an Entry's
    /// `mtime`, which is legal before 1970 (FM-9).
    Int {
        /// The number itself.
        value: i64,
    },
    /// A text string.
    Text {
        /// The text itself.
        value: String,
    },
    /// A byte string, spelled as lowercase hex.
    Bytes {
        /// The bytes, hex-encoded.
        value: String,
    },
    /// An array, whose order is part of what the manifest states.
    Array {
        /// The elements, in order.
        value: Vec<BodyValue>,
    },
    /// A nested map, described the same way the body itself is.
    Map {
        /// The fields, in any order: a map is compared by name.
        value: Vec<BodyField>,
    },
}

impl BodyValue {
    /// This value as the CBOR item a payload carries.
    pub(super) fn to_cbor(&self) -> Result<Value> {
        Ok(match self {
            Self::Uint { value } => Value::from(*value),
            Self::Int { value } => Value::from(*value),
            Self::Text { value } => Value::Text(value.clone()),
            Self::Bytes { value } => Value::Bytes(hex::decode(value)?),
            Self::Array { value } => Value::Array(
                value
                    .iter()
                    .map(Self::to_cbor)
                    .collect::<Result<Vec<_>>>()?,
            ),
            Self::Map { value } => Value::Map(
                value
                    .iter()
                    .map(|field| Ok((Value::Text(field.key.clone()), field.to_cbor()?)))
                    .collect::<Result<Vec<_>>>()?,
            ),
        })
    }
}
