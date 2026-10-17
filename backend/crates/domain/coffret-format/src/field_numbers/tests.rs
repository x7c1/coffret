//! The field-number declarations against the registry, and the translation's
//! own rules
//! (spec: FM-9, FM-15, FM-16, FM-17).

use std::collections::{BTreeMap, BTreeSet};

use ciborium::Value;

use super::registered_field::{field, RegisteredField};
use super::*;

/// One registry row, as the declarations and the file both state it.
type Row = (String, u64, bool);

/// The registry file, read the way the specification states it.
fn registry() -> BTreeMap<String, BTreeSet<Row>> {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../docs/spec/format/field-numbers.json"
    );
    let text = std::fs::read_to_string(path).expect("the field-number registry is readable");
    let json: serde_json::Value =
        serde_json::from_str(&text).expect("the field-number registry is JSON");
    let mut maps: BTreeMap<String, BTreeSet<Row>> = BTreeMap::new();
    for row in json["fields"]
        .as_array()
        .expect("the registry lists its fields")
    {
        let map = row["map"].as_str().expect("a row names its map");
        let name = row["name"].as_str().expect("a row names its field");
        let number = row["number"]
            .as_u64()
            .expect("a row gives an unsigned field number");
        let retired = row
            .get("retired")
            .map(|retired| retired.as_bool().expect("retired is a boolean"))
            .unwrap_or(false);
        maps.entry(map.to_owned())
            .or_default()
            .insert((name.to_owned(), number, retired));
    }
    maps
}

/// What this crate declares, in the registry's shape.
fn declared() -> BTreeMap<String, BTreeSet<Row>> {
    ALL.iter()
        .map(|kind| {
            (
                kind.map.to_owned(),
                kind.fields
                    .iter()
                    .map(|field| (field.name.to_owned(), field.number, field.retired))
                    .collect(),
            )
        })
        .collect()
}

#[test]
fn every_map_this_crate_declares_matches_the_registry_in_both_directions() {
    let registry = registry();
    let declared = declared();
    assert_eq!(
        registry.keys().collect::<Vec<_>>(),
        declared.keys().collect::<Vec<_>>(),
        "the registry and this crate declare different map kinds"
    );
    for (map, rows) in &registry {
        let ours = &declared[map];
        let missing: Vec<_> = rows.difference(ours).collect();
        let extra: Vec<_> = ours.difference(rows).collect();
        assert!(
            missing.is_empty() && extra.is_empty(),
            "map {map}: the registry has {missing:?} that this crate does not declare, \
             and this crate declares {extra:?} that the registry does not have"
        );
    }
}

// Every map a field holds is a kind of its own in the list the registry is
// checked against, so no nested map escapes that check.
#[test]
fn every_nested_map_is_a_declared_kind() {
    for kind in ALL {
        for field in kind.fields {
            if let Some(nested) = field.holds {
                assert!(
                    ALL.iter().any(|declared| std::ptr::eq(*declared, nested)),
                    "{}.{} holds {}, which is not in ALL",
                    kind.map,
                    field.name,
                    nested.map
                );
            }
        }
    }
}

#[test]
fn within_one_map_every_field_number_is_unique_and_every_live_name_too() {
    for kind in ALL {
        let mut seen = BTreeSet::new();
        let mut live = BTreeSet::new();
        for field in kind.fields {
            assert!(
                seen.insert(field.number),
                "{} uses field number {} twice",
                kind.map,
                field.number
            );
            if !field.retired {
                assert!(
                    live.insert(field.name),
                    "{} has two live fields named {}",
                    kind.map,
                    field.name
                );
            }
        }
    }
}

// The registry's own rows, held to the same rule: a field number is never
// reused within its map, retired or not.
#[test]
fn the_registry_uses_no_field_number_twice_within_a_map() {
    for (map, rows) in registry() {
        let numbers: Vec<u64> = rows.iter().map(|(_, number, _)| *number).collect();
        let unique: BTreeSet<u64> = numbers.iter().copied().collect();
        assert_eq!(
            numbers.len(),
            unique.len(),
            "map {map} uses a field number twice"
        );
    }
}

// FM-13: the framing finds the epoch before it knows which kind's map it holds,
// so every payload map carries it under the one field number the framing reads.
#[test]
fn every_payload_map_carries_the_epoch_under_the_framings_field_number() {
    for kind in [&JOURNAL_RECORD, &INDEX_SNAPSHOT, &KEYRING] {
        assert_eq!(
            kind.number_of("master_key_epoch"),
            Some(MASTER_KEY_EPOCH_FIELD),
            "{}",
            kind.map
        );
    }
}

/// A map kind with one retired field and the live one that replaced it.
static WITH_RETIRED: MapFields = MapFields {
    map: "with_retired",
    fields: &[
        field("kept", 0),
        RegisteredField {
            name: "replaced",
            number: 1,
            retired: true,
            holds: None,
        },
        field("replaced", 2),
    ],
};

fn named(fields: &[(&str, u64)]) -> Value {
    Value::Map(
        fields
            .iter()
            .map(|(name, value)| (Value::Text((*name).to_owned()), Value::from(*value)))
            .collect(),
    )
}

fn wire(fields: &[(u64, u64)]) -> Value {
    Value::Map(
        fields
            .iter()
            .map(|(number, value)| (Value::from(*number), Value::from(*value)))
            .collect(),
    )
}

#[test]
fn a_retired_field_number_is_never_read() {
    let read = from_wire(wire(&[(0, 10), (1, 11)]), &WITH_RETIRED);
    assert_eq!(read, named(&[("kept", 10)]));
}

#[test]
fn a_retired_field_number_is_never_written() {
    let written = to_wire(named(&[("kept", 10), ("replaced", 12)]), &WITH_RETIRED)
        .expect("both names are live");
    assert_eq!(written, wire(&[(0, 10), (2, 12)]));
    assert_eq!(WITH_RETIRED.number_of("replaced"), Some(2));
}

// FM-9: the maps are forward-open, so a field number this build does not know
// is stepped over rather than refused.
#[test]
fn an_unknown_field_number_is_dropped_on_the_way_in() {
    let read = from_wire(wire(&[(0, 10), (23, 1), (1_000, 2)]), &WITH_RETIRED);
    assert_eq!(read, named(&[("kept", 10)]));
}

// A text key is no field's spelling: one that happens to spell a field's name
// is dropped like any other map key this build does not know, so the reader
// finds the field missing.
#[test]
fn a_text_key_is_no_fields_spelling() {
    let read = from_wire(named(&[("kept", 10)]), &WITH_RETIRED);
    assert_eq!(read, Value::Map(Vec::new()));
}

#[test]
fn a_name_the_map_does_not_declare_is_refused_on_the_way_out() {
    let refused = to_wire(named(&[("unheard_of", 1)]), &WITH_RETIRED)
        .expect_err("an undeclared name has no field number");
    let crate::Error::UnregisteredField { map, field } = &refused else {
        panic!("expected the field to be unregistered, got {refused:?}");
    };
    assert_eq!(*map, "with_retired");
    assert_eq!(field, "unheard_of");
}

#[test]
fn nested_maps_are_translated_by_their_own_kind() {
    let named_value = Value::Map(vec![
        (Value::Text("schema".to_owned()), Value::from(2u64)),
        (
            Value::Text("additions".to_owned()),
            Value::Array(vec![Value::Map(vec![(
                Value::Text("entries".to_owned()),
                Value::Array(vec![Value::Map(vec![(
                    Value::Text("captured_path".to_owned()),
                    Value::Text("a".to_owned()),
                )])]),
            )])]),
        ),
    ]);
    let wire_value = to_wire(named_value.clone(), &JOURNAL_RECORD).expect("every name is declared");
    let expected = Value::Map(vec![
        (Value::from(1u64), Value::from(2u64)),
        (
            Value::from(8u64),
            Value::Array(vec![Value::Map(vec![(
                Value::from(5u64),
                Value::Array(vec![Value::Map(vec![(
                    Value::from(8u64),
                    Value::Text("a".to_owned()),
                )])]),
            )])]),
        ),
    ]);
    assert_eq!(wire_value, expected);
    assert_eq!(from_wire(wire_value, &JOURNAL_RECORD), named_value);
}

// The view a person reads keeps what a decoder drops, and spells it so that it
// cannot pass for a field.
#[test]
fn the_named_view_keeps_what_it_cannot_name() {
    let raw = Value::Map(vec![
        (Value::from(0u64), Value::from(10u64)),
        (Value::from(1u64), Value::from(11u64)),
        (Value::from(9u64), Value::from(12u64)),
        (Value::Text("kept".to_owned()), Value::from(13u64)),
    ]);
    let view = named_view(raw, &WITH_RETIRED);
    assert_eq!(
        view,
        Value::Map(vec![
            (Value::Text("kept".to_owned()), Value::from(10u64)),
            (Value::from(1u64), Value::from(11u64)),
            (Value::from(9u64), Value::from(12u64)),
            (Value::Text("\"kept\"".to_owned()), Value::from(13u64)),
        ])
    );
}
