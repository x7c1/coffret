use serde::{Deserialize, Serialize};

/// The value types a manifest may state for a body field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BodyValue {
    /// An unsigned integer.
    Uint {
        /// The number itself.
        value: u64,
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
}
