use super::wire_entry::WireEntry;
use super::wire_kind::WireKind;
use super::wire_meta::WireMeta;
use super::{Meta, SCHEMA};
use crate::error::{Error, Result};

/// Serializes a meta section to its CBOR plaintext.
pub(crate) fn encode(meta: &Meta) -> Result<Vec<u8>> {
    let wire = WireMeta {
        schema: SCHEMA,
        kind: WireKind::from(meta.kind),
        pad_len: meta.pad_len,
        entries: meta.entries.iter().map(WireEntry::from).collect(),
    };
    let mut bytes = Vec::new();
    ciborium::into_writer(&wire, &mut bytes).map_err(|error| Error::MetaEncodeFailed {
        detail: error.to_string(),
    })?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use coffret_model::ContainerKind;

    use super::super::testing::{as_value, sample};

    // FM-9: the meta section is one CBOR map with `schema`, `kind`, `pad_len`,
    // and `entries`; each entry records `path`, `offset`, `size`, `mtime`, and
    // `hash`, plus optional `derived_from` and `mime`.
    #[test]
    fn field_names_and_kind_spelling_match_the_rule() {
        let value = as_value(&sample());
        let map = value.as_map().expect("the meta section is a CBOR map");
        let keys: Vec<&str> = map
            .iter()
            .map(|(key, _)| key.as_text().expect("keys are text"))
            .collect();
        assert_eq!(keys, ["schema", "kind", "pad_len", "entries"]);

        let container_kind = map
            .iter()
            .find(|(key, _)| key.as_text() == Some("kind"))
            .map(|(_, value)| value)
            .expect("kind is present");
        assert_eq!(container_kind.as_text(), Some("pack"));

        let entries = map
            .iter()
            .find(|(key, _)| key.as_text() == Some("entries"))
            .map(|(_, value)| value.as_array().expect("entries is an array"))
            .expect("entries is present");
        let entry_keys: Vec<&str> = entries[0]
            .as_map()
            .expect("an entry is a CBOR map")
            .iter()
            .map(|(key, _)| key.as_text().expect("keys are text"))
            .collect();
        assert_eq!(entry_keys, ["path", "offset", "size", "mtime", "hash"]);
    }

    // FM-9: the other spelling `kind` carries is `one-file`.
    #[test]
    fn one_file_kind_spelling_matches_the_rule() {
        let mut meta = sample();
        meta.kind = ContainerKind::OneFile;
        let value = as_value(&meta);
        let map = value.as_map().expect("the meta section is a CBOR map");
        let container_kind = map
            .iter()
            .find(|(key, _)| key.as_text() == Some("kind"))
            .map(|(_, value)| value)
            .expect("kind is present");
        assert_eq!(container_kind.as_text(), Some("one-file"));
    }
}
