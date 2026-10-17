use ciborium::Value;

use super::field_number::field_number;
use super::map_fields::MapFields;

/// A value read off the wire, keyed by field names for a person reading it.
///
/// Where `from_wire` drops what it does not know, this keeps it, so a dumped
/// payload shows everything it carries: a field number the registry names for
/// this map is shown as that name, one it does not name (unknown or retired) is
/// left as the integer, and a text map key — which is no field's spelling — is
/// shown quoted, so it can never be mistaken for the field it spells.
pub fn named_view(value: Value, kind: &MapFields) -> Value {
    match value {
        Value::Map(entries) => Value::Map(
            entries
                .into_iter()
                .map(|(key, value)| {
                    match field_number(&key).and_then(|number| kind.live_by_number(number)) {
                        Some(field) => (
                            Value::Text(field.name.to_owned()),
                            match field.holds {
                                Some(nested) => named_view(value, nested),
                                None => value,
                            },
                        ),
                        None => match key {
                            Value::Text(text) => (Value::Text(format!("{text:?}")), value),
                            other => (other, value),
                        },
                    }
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| named_view(item, kind))
                .collect(),
        ),
        other => other,
    }
}
