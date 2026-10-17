use ciborium::Value;

use super::map_fields::MapFields;

/// [`to_wire`](fn@super::to_wire) for a test that builds a map by hand: a name
/// this map kind declares becomes its field number, and any other map key is
/// kept as it is, so a case can put an unknown field number — or a text key —
/// beside the fields it names.
pub(crate) fn to_wire_lenient(value: Value, kind: &MapFields) -> Value {
    match value {
        Value::Map(entries) => Value::Map(
            entries
                .into_iter()
                .map(
                    |(key, value)| match key.as_text().and_then(|name| kind.live_by_name(name)) {
                        Some(field) => (
                            Value::from(field.number),
                            match field.holds {
                                Some(nested) => to_wire_lenient(value, nested),
                                None => value,
                            },
                        ),
                        None => (key, value),
                    },
                )
                .collect(),
        ),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| to_wire_lenient(item, kind))
                .collect(),
        ),
        other => other,
    }
}
