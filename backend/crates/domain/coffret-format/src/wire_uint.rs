use coffret_model::MAX_FORMAT_INTEGER;
use serde::de::Error as _;
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// One unsigned integer of a serde-deserialized wire map, held to the bound the
/// format puts on every integer it carries (spec: FM-19).
///
/// The meta section's maps and the entry maps a control payload carries are
/// deserialized as whole structs rather than read field by field, so a plain
/// `u64` field would take the full 64-bit range whatever the rule says. Every
/// unsigned field of those maps is this type instead — `schema`, `pad_len`,
/// `offset`, `size` — which states the bound once for all of them and reports a
/// number past it as the malformed map it makes, naming what was expected.
///
/// The payload fields read one at a time do not come through here: they go
/// through the control readers' own `Fields`, which states the same bound where
/// a CBOR item becomes a number.
///
/// The bound is checked in both directions. A writer never has a larger number
/// to write — the entry table's extents were built against the same bound, and
/// a schema and a padding length are its own arithmetic — so the serializing
/// half is a guarantee rather than a path a caller takes, and it fails loudly
/// instead of putting an object on Storage that this crate's own reader would
/// refuse.
#[derive(Clone, Copy)]
pub(crate) struct WireUint(u64);

impl WireUint {
    /// The number, for a caller that has read it off the wire.
    pub(crate) const fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for WireUint {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

/// What both halves say a number outside the range is.
fn out_of_range(value: u64) -> String {
    format!("expected an unsigned integer below 2^63, found {value}")
}

impl Serialize for WireUint {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.0 > MAX_FORMAT_INTEGER {
            return Err(S::Error::custom(out_of_range(self.0)));
        }
        serializer.serialize_u64(self.0)
    }
}

impl<'de> Deserialize<'de> for WireUint {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = u64::deserialize(deserializer)?;
        if value > MAX_FORMAT_INTEGER {
            return Err(D::Error::custom(out_of_range(value)));
        }
        Ok(Self(value))
    }
}
