use ciborium::Value;

use super::map_fields::MapFields;
use crate::error::{Error, Result};

/// A value keyed by field names, as the wire carries it: every map of `kind` —
/// `value` itself, or each map in it where it is an array — keyed by field
/// numbers instead, and every map nested in one by its own kind's.
///
/// The refusal is [`Error::UnregisteredField`], naming the field and the map
/// kind that has no live field number for it, which only a writer bug produces.
pub(crate) fn to_wire(value: Value, kind: &MapFields) -> Result<Value> {
    match value {
        Value::Map(entries) => entries
            .into_iter()
            .map(|(name, value)| {
                let Some(field) = name.as_text().and_then(|name| kind.live_by_name(name)) else {
                    return Err(Error::UnregisteredField {
                        map: kind.map,
                        field: match name.into_text() {
                            Ok(name) => name,
                            Err(other) => format!("{other:?}"),
                        },
                    });
                };
                let value = match field.holds {
                    Some(nested) => to_wire(value, nested)?,
                    None => value,
                };
                Ok((Value::from(field.number), value))
            })
            .collect::<Result<Vec<_>>>()
            .map(Value::Map),
        Value::Array(items) => items
            .into_iter()
            .map(|item| to_wire(item, kind))
            .collect::<Result<Vec<_>>>()
            .map(Value::Array),
        other => Ok(other),
    }
}
