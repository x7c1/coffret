//! Moving a payload body between the fields a manifest states and the CBOR map
//! a control object carries.

use anyhow::{bail, Context, Result};
use ciborium::Value;

use super::{BodyField, BodyValue};

/// Serializes the fields a manifest states into the CBOR map a payload carries.
pub fn to_cbor_map(fields: &[BodyField]) -> Result<Vec<u8>> {
    let entries = fields
        .iter()
        .map(|field| Ok((Value::Text(field.key.clone()), field.to_cbor()?)))
        .collect::<Result<Vec<_>>>()?;

    let mut bytes = Vec::new();
    ciborium::into_writer(&Value::Map(entries), &mut bytes)
        .context("serializing a control-payload body")?;
    Ok(bytes)
}

/// Checks a decoded payload body against the fields a manifest states.
///
/// The comparison is on decoded CBOR, never on bytes: the two implementations
/// order and spell map entries as they please, and only the fields are
/// normative. That holds at every level, so a nested map is compared by field
/// name too — while an array is compared in order, because the order of every
/// array in a payload is part of what its rule states (FM-15, FM-16).
pub fn check_cbor_map(bytes: &[u8], expected: &[BodyField]) -> Result<()> {
    let value: Value =
        ciborium::from_reader(bytes).context("the payload body is not readable CBOR")?;
    check_map(&value, expected, "the payload body")
}

fn check_map(found: &Value, expected: &[BodyField], what: &str) -> Result<()> {
    let Value::Map(entries) = found else {
        bail!("{what} is not a CBOR map");
    };
    if entries.len() != expected.len() {
        bail!(
            "{what} carries {} fields, the manifest states {}",
            entries.len(),
            expected.len()
        );
    }
    for field in expected {
        let found = entries
            .iter()
            .find(|(key, _)| key.as_text() == Some(field.key.as_str()))
            .map(|(_, value)| value)
            .with_context(|| format!("{what} carries no field {:?}", field.key))?;
        check_value(
            found,
            &field.value,
            &format!("{what} field {:?}", field.key),
        )?;
    }
    Ok(())
}

fn check_value(found: &Value, expected: &BodyValue, what: &str) -> Result<()> {
    match expected {
        BodyValue::Array { value } => {
            let Value::Array(items) = found else {
                bail!("{what} is not an array");
            };
            if items.len() != value.len() {
                bail!(
                    "{what} holds {} elements, the manifest states {}",
                    items.len(),
                    value.len()
                );
            }
            for (index, (item, expected)) in items.iter().zip(value).enumerate() {
                check_value(item, expected, &format!("{what}[{index}]"))?;
            }
            Ok(())
        }
        BodyValue::Map { value } => check_map(found, value, what),
        scalar => {
            let wanted = scalar.to_cbor()?;
            if *found != wanted {
                bail!("{what} decoded to {found:?}, the manifest states {wanted:?}");
            }
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields() -> Vec<BodyField> {
        vec![
            BodyField::uint("records", 2),
            BodyField::text("note", "a body the framing never reads"),
            BodyField::bytes("digest", &[0xa1, 0xb2]),
        ]
    }

    /// A body shaped like the payload schemas: an array of maps (FM-15, FM-16).
    fn nested() -> Vec<BodyField> {
        vec![
            BodyField::uint("schema", 1),
            BodyField::array(
                "additions",
                vec![
                    BodyValue::Map {
                        value: vec![
                            BodyField::bytes("id", &[0x21; 16]),
                            BodyField::int("mtime", -2_208_988_800),
                        ],
                    },
                    BodyValue::Map {
                        value: vec![BodyField::bytes("id", &[0x40; 16])],
                    },
                ],
            ),
        ]
    }

    #[test]
    fn a_body_the_manifest_describes_checks_out() {
        let bytes = to_cbor_map(&fields()).expect("the fields serialize");
        check_cbor_map(&bytes, &fields()).expect("the body matches the manifest");
    }

    #[test]
    fn a_nested_body_the_manifest_describes_checks_out() {
        let bytes = to_cbor_map(&nested()).expect("the fields serialize");
        check_cbor_map(&bytes, &nested()).expect("the body matches the manifest");
    }

    // The two encoders order map keys as they please, so the check must not
    // depend on the order the fields arrive in — at any level.
    #[test]
    fn field_order_does_not_matter() {
        let mut reordered = fields();
        reordered.reverse();
        let bytes = to_cbor_map(&reordered).expect("the fields serialize");
        check_cbor_map(&bytes, &fields()).expect("the body matches the manifest");
    }

    #[test]
    fn field_order_inside_a_nested_map_does_not_matter() {
        let mut written = nested();
        let BodyValue::Array { value } = &mut written[1].value else {
            panic!("additions is an array");
        };
        let BodyValue::Map { value } = &mut value[0] else {
            panic!("an addition is a map");
        };
        value.reverse();
        let bytes = to_cbor_map(&written).expect("the fields serialize");
        check_cbor_map(&bytes, &nested()).expect("the body matches the manifest");
    }

    // Array order, on the other hand, is exactly what FM-15 and FM-16 fix, so a
    // body whose additions arrived in the other order is one the exchange must
    // not wave through.
    #[test]
    fn array_order_does_matter() {
        let mut written = nested();
        let BodyValue::Array { value } = &mut written[1].value else {
            panic!("additions is an array");
        };
        value.reverse();
        let bytes = to_cbor_map(&written).expect("the fields serialize");
        let error = check_cbor_map(&bytes, &nested()).expect_err("the order differs");
        assert!(format!("{error:#}").contains("additions"), "{error:#}");
    }

    #[test]
    fn a_missing_field_is_reported_by_name() {
        let mut written = fields();
        written[2] = BodyField::bytes("checksum", &[0xa1, 0xb2]);
        let bytes = to_cbor_map(&written).expect("the fields serialize");
        let error = check_cbor_map(&bytes, &fields()).expect_err("a field is missing");
        assert!(format!("{error:#}").contains("digest"), "{error:#}");
    }

    #[test]
    fn a_missing_field_of_a_nested_map_is_reported_by_name() {
        let mut written = nested();
        let BodyValue::Array { value } = &mut written[1].value else {
            panic!("additions is an array");
        };
        let BodyValue::Map { value } = &mut value[0] else {
            panic!("an addition is a map");
        };
        value[1] = BodyField::int("modified", -1);
        let bytes = to_cbor_map(&written).expect("the fields serialize");
        let error = check_cbor_map(&bytes, &nested()).expect_err("a field is missing");
        assert!(format!("{error:#}").contains("mtime"), "{error:#}");
    }

    // A body that dropped a field is a body the writer and the reader disagree
    // about, even when every field the reader looks for is still there.
    #[test]
    fn a_body_with_a_different_field_count_is_rejected() {
        let mut written = fields();
        written.pop();
        let bytes = to_cbor_map(&written).expect("the fields serialize");
        let error = check_cbor_map(&bytes, &fields()).expect_err("a field is missing");
        assert!(format!("{error:#}").contains("2 fields"), "{error:#}");
    }

    #[test]
    fn a_changed_value_is_reported_by_name() {
        let mut written = fields();
        written[0] = BodyField::uint("records", 3);
        let bytes = to_cbor_map(&written).expect("the fields serialize");
        let error = check_cbor_map(&bytes, &fields()).expect_err("a value differs");
        assert!(format!("{error:#}").contains("records"), "{error:#}");
    }

    #[test]
    fn an_array_of_a_different_length_is_rejected() {
        let mut written = nested();
        let BodyValue::Array { value } = &mut written[1].value else {
            panic!("additions is an array");
        };
        value.pop();
        let bytes = to_cbor_map(&written).expect("the fields serialize");
        let error = check_cbor_map(&bytes, &nested()).expect_err("an element is missing");
        assert!(format!("{error:#}").contains("1 element"), "{error:#}");
    }
}
