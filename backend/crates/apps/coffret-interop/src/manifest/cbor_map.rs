//! Moving a payload body between the fields a manifest states and the CBOR map
//! a control object carries.

use anyhow::{bail, Context, Result};
use ciborium::Value;

use super::BodyField;

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
/// normative.
pub fn check_cbor_map(bytes: &[u8], expected: &[BodyField]) -> Result<()> {
    let value: Value =
        ciborium::from_reader(bytes).context("the payload body is not readable CBOR")?;
    let Value::Map(entries) = value else {
        bail!("the payload body is not a CBOR map");
    };
    if entries.len() != expected.len() {
        bail!(
            "the payload body carries {} fields, the manifest states {}",
            entries.len(),
            expected.len()
        );
    }
    for field in expected {
        let found = entries
            .iter()
            .find(|(key, _)| key.as_text() == Some(field.key.as_str()))
            .map(|(_, value)| value)
            .with_context(|| format!("the payload body carries no field {:?}", field.key))?;
        let wanted = field.to_cbor()?;
        if *found != wanted {
            bail!(
                "field {:?} decoded to {:?}, the manifest states {:?}",
                field.key,
                found,
                wanted
            );
        }
    }
    Ok(())
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

    #[test]
    fn a_body_the_manifest_describes_checks_out() {
        let bytes = to_cbor_map(&fields()).expect("the fields serialize");
        check_cbor_map(&bytes, &fields()).expect("the body matches the manifest");
    }

    // The two encoders order map keys as they please, so the check must not
    // depend on the order the fields arrive in.
    #[test]
    fn field_order_does_not_matter() {
        let mut reordered = fields();
        reordered.reverse();
        let bytes = to_cbor_map(&reordered).expect("the fields serialize");
        check_cbor_map(&bytes, &fields()).expect("the body matches the manifest");
    }

    #[test]
    fn a_missing_field_is_reported_by_name() {
        let mut written = fields();
        written[2] = BodyField::bytes("checksum", &[0xa1, 0xb2]);
        let bytes = to_cbor_map(&written).expect("the fields serialize");
        let error = check_cbor_map(&bytes, &fields()).expect_err("a field is missing");
        assert!(format!("{error:#}").contains("digest"), "{error:#}");
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
}
