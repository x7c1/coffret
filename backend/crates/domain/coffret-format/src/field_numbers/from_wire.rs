use ciborium::Value;

use super::field_number::field_number;
use super::map_fields::MapFields;

/// A value read off the wire, keyed by field names for a reader: every field
/// number this build knows becomes its field's name, and every other map key —
/// an unknown field number, a retired one, any text — is dropped (spec: FM-9).
///
/// A value of another shape than `kind` expects is left as it is, for the
/// reader to refuse by the field it went wrong at.
pub(crate) fn from_wire(value: Value, kind: &MapFields) -> Value {
    match value {
        Value::Map(entries) => Value::Map(
            entries
                .into_iter()
                .filter_map(|(key, value)| {
                    let field =
                        field_number(&key).and_then(|number| kind.live_by_number(number))?;
                    let value = match field.holds {
                        Some(nested) => from_wire(value, nested),
                        None => value,
                    };
                    Some((Value::Text(field.name.to_owned()), value))
                })
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| from_wire(item, kind))
                .collect(),
        ),
        other => other,
    }
}
