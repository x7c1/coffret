use ciborium::Value;
use coffret_model::{CiphertextLenClaim, Generation};

use super::{as_bounded_uint, describe};
use crate::error::{Error, Result};

/// The fields of one CBOR map, read as the schema that owns it spells them.
pub(in crate::control) struct Fields<'a> {
    entries: &'a [(Value, Value)],
    /// What a field of the wrong shape is reported as: FM-15's map and FM-16's
    /// raise different errors for the same misreading.
    malformed: fn(String) -> Error,
}

impl<'a> Fields<'a> {
    /// Takes the map a payload schema is written as, rejecting any other item.
    pub(in crate::control) fn of(value: &'a Value, malformed: fn(String) -> Error) -> Result<Self> {
        match value {
            Value::Map(entries) => Ok(Self { entries, malformed }),
            other => Err(malformed(format!("{} is not a map", describe(other)))),
        }
    }

    /// The value at `key`, or `None` where the map does not carry it.
    ///
    /// The maps are forward-open (FM-9), so a key this build does not know is
    /// simply never asked for.
    pub(in crate::control) fn get(&self, key: &str) -> Option<&'a Value> {
        self.entries
            .iter()
            .find(|(name, _)| name.as_text() == Some(key))
            .map(|(_, value)| value)
    }

    /// A field the schema declares as an unsigned 64-bit integer.
    pub(in crate::control) fn uint(&self, key: &str) -> Result<u64> {
        self.require(key).and_then(|value| self.as_uint(key, value))
    }

    /// An optional field the schema declares as an unsigned 64-bit integer.
    pub(in crate::control) fn optional_uint(&self, key: &str) -> Result<Option<u64>> {
        self.get(key)
            .map(|value| self.as_uint(key, value))
            .transpose()
    }

    /// A field the schema declares as a generation number (FM-13).
    pub(in crate::control) fn generation(&self, key: &str) -> Result<Generation> {
        let number = self.uint(key)?;
        self.bounded(key, Generation::new(number))
    }

    /// An optional field the schema declares as a generation number.
    pub(in crate::control) fn optional_generation(&self, key: &str) -> Result<Option<Generation>> {
        self.optional_uint(key)?
            .map(|number| self.bounded(key, Generation::new(number)))
            .transpose()
    }

    /// A field the schema declares as a Container's claimed ciphertext length
    /// (CP-11).
    pub(in crate::control) fn ciphertext_len(&self, key: &str) -> Result<CiphertextLenClaim> {
        let number = self.uint(key)?;
        self.bounded(key, CiphertextLenClaim::new(number))
    }

    /// A field the schema declares as an unsigned integer no wider than `u16`.
    pub(in crate::control) fn u16(&self, key: &str) -> Result<u16> {
        let value = self.uint(key)?;
        u16::try_from(value).map_err(|_| (self.malformed)(format!("{key} is {value}, past 65535")))
    }

    /// An optional field the schema declares as a boolean.
    ///
    /// The one such field is FM-17's `key_lost`, whose presence is the marker.
    /// Its value is read all the same: the schema spells the marker `true`, and
    /// a reader that took any value there as a marker would accept two
    /// spellings of it.
    pub(in crate::control) fn optional_bool(&self, key: &str) -> Result<Option<bool>> {
        self.get(key)
            .map(|value| {
                value.as_bool().ok_or_else(|| {
                    (self.malformed)(format!("{key} is a boolean, found {}", describe(value)))
                })
            })
            .transpose()
    }

    /// A field the schema declares as text.
    pub(in crate::control) fn text(&self, key: &str) -> Result<String> {
        self.require(key).and_then(|value| self.as_text(key, value))
    }

    /// An optional field the schema declares as text.
    pub(in crate::control) fn optional_text(&self, key: &str) -> Result<Option<String>> {
        self.get(key)
            .map(|value| self.as_text(key, value))
            .transpose()
    }

    /// A field the schema declares as a byte string of exactly `len` bytes.
    pub(in crate::control) fn byte_array<const LEN: usize>(&self, key: &str) -> Result<[u8; LEN]> {
        let bytes = match self.require(key)? {
            Value::Bytes(bytes) => bytes,
            other => {
                return Err((self.malformed)(format!(
                    "{key} is a byte string, found {}",
                    describe(other)
                )))
            }
        };
        <[u8; LEN]>::try_from(bytes.as_slice()).map_err(|_| {
            Error::Model(coffret_model::Error::InvalidByteLength {
                expected: LEN,
                actual: bytes.len(),
            })
        })
    }

    /// A field the schema declares as an array.
    pub(in crate::control) fn array(&self, key: &str) -> Result<&'a [Value]> {
        match self.require(key)? {
            Value::Array(items) => Ok(items),
            other => Err((self.malformed)(format!(
                "{key} is an array, found {}",
                describe(other)
            ))),
        }
    }

    /// The fields of a map this map carries at `key`.
    pub(in crate::control) fn map(&self, value: &'a Value) -> Result<Self> {
        Self::of(value, self.malformed)
    }

    fn require(&self, key: &str) -> Result<&'a Value> {
        self.get(key)
            .ok_or_else(|| (self.malformed)(format!("{key} is missing")))
    }

    /// One unsigned field, held to the bound FM-19 puts on every integer the
    /// format carries.
    ///
    /// Every unsigned field of every payload schema is read through here, so
    /// the bound is stated once for all of them: the number a field carries is
    /// below 2^63 or the payload is malformed, whichever field it was. A
    /// number past the bound is named in the detail — it is the format's own
    /// arithmetic and says nothing about the Library's content — while a value
    /// of the wrong shape is only described, since a text field's content is
    /// not this layer's to quote.
    fn as_uint(&self, key: &str, value: &Value) -> Result<u64> {
        as_bounded_uint(value).ok_or_else(|| {
            let found = match value
                .as_integer()
                .and_then(|integer| u64::try_from(integer).ok())
            {
                Some(number) => number.to_string(),
                None => describe(value).to_owned(),
            };
            (self.malformed)(format!(
                "{key} is an unsigned integer below 2^63, found {found}"
            ))
        })
    }

    /// A domain value made out of a field this map has already read, or this
    /// schema's own refusal where the type would not take it.
    ///
    /// The types these fields become hold themselves to the bound FM-19 puts on
    /// every integer the format carries, which is the bound
    /// [`as_uint`](Self::as_uint) has just held the number to, so nothing is
    /// left for them to refuse. This is where that agreement is written down: a
    /// number no payload field can carry is this schema's malformed map however
    /// it is caught, and a reader that has already stated the rule does not
    /// hand its caller a second spelling of the same refusal. The refused
    /// value is not lost — the type's own account of it goes into the detail,
    /// which is what would say what parted if the two bounds ever did.
    fn bounded<T>(&self, key: &str, value: coffret_model::Result<T>) -> Result<T> {
        value.map_err(|error| {
            (self.malformed)(format!("{key} is no number this schema carries: {error}"))
        })
    }

    fn as_text(&self, key: &str, value: &Value) -> Result<String> {
        value
            .as_text()
            .map(str::to_owned)
            .ok_or_else(|| (self.malformed)(format!("{key} is text, found {}", describe(value))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control::cbor::MapBuilder;
    use coffret_model::MAX_FORMAT_INTEGER;

    fn malformed(detail: String) -> Error {
        Error::MalformedJournalRecord { detail }
    }

    fn sample() -> Value {
        MapBuilder::new()
            .uint("schema", 1)
            .text("note", "hello")
            .bytes("id", &[1, 2, 3])
            .value("list", Value::Array(vec![Value::from(1u64)]))
            .build()
    }

    #[test]
    fn fields_are_read_by_name_whatever_order_they_are_in() {
        let value = sample();
        let fields = Fields::of(&value, malformed).expect("the sample is a map");
        assert_eq!(fields.uint("schema").expect("schema reads"), 1);
        assert_eq!(fields.text("note").expect("note reads"), "hello");
        assert_eq!(fields.array("list").expect("list reads").len(), 1);
        assert_eq!(fields.optional_text("absent").expect("absent reads"), None);
    }

    #[test]
    fn a_missing_required_field_is_named() {
        let value = sample();
        let fields = Fields::of(&value, malformed).expect("the sample is a map");
        let result = fields.uint("removals");
        assert!(
            matches!(result, Err(Error::MalformedJournalRecord { ref detail }) if detail.contains("removals")),
            "expected the missing field to be named, got {result:?}"
        );
    }

    #[test]
    fn a_field_of_the_wrong_shape_is_named_without_quoting_it() {
        let value = sample();
        let fields = Fields::of(&value, malformed).expect("the sample is a map");
        let result = fields.uint("note");
        let Err(Error::MalformedJournalRecord { detail }) = result else {
            panic!("expected text under an unsigned field to be rejected, got {result:?}");
        };
        assert!(detail.contains("note"), "{detail}");
        assert!(!detail.contains("hello"), "{detail}");
    }

    // FM-19: every unsigned integer a control payload carries is below 2^63,
    // whichever field carries it, so one field's check is every field's. The
    // detail names the key and the number: both are the format's own
    // arithmetic and neither says anything about the Library's content.
    #[test]
    fn a_payload_integer_past_the_formats_integer_range_is_malformed() {
        let value = MapBuilder::new()
            .uint("head_generation", MAX_FORMAT_INTEGER + 1)
            .uint("journal_generation", MAX_FORMAT_INTEGER)
            .build();
        let fields = Fields::of(&value, malformed).expect("the map is a map");

        let result = fields.uint("head_generation");
        let Err(Error::MalformedJournalRecord { detail }) = result else {
            panic!("expected an integer of 2^63 to be refused, got {result:?}");
        };
        assert!(detail.contains("head_generation"), "{detail}");
        assert!(detail.contains("below 2^63"), "{detail}");
        assert!(
            detail.contains(&(MAX_FORMAT_INTEGER + 1).to_string()),
            "{detail}"
        );

        assert_eq!(
            fields
                .uint("journal_generation")
                .expect("the bound itself reads"),
            MAX_FORMAT_INTEGER,
        );
    }

    // A byte string of the wrong length is a domain error rather than a schema
    // one: the field was the shape the schema gives it and the value it carried
    // is not one the type accepts.
    #[test]
    fn a_byte_string_of_the_wrong_length_is_rejected() {
        let value = sample();
        let fields = Fields::of(&value, malformed).expect("the sample is a map");
        let result = fields.byte_array::<16>("id");
        assert!(
            matches!(
                result,
                Err(Error::Model(coffret_model::Error::InvalidByteLength {
                    expected: 16,
                    actual: 3
                }))
            ),
            "expected a 3-byte value under a 16-byte field to be rejected, got {result:?}"
        );
    }
}
