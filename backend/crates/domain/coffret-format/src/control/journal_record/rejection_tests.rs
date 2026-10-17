//! Journal record payloads a reader refuses (spec: FM-15).

use crate::field_numbers;
use ciborium::Value;
use coffret_model::{Generation, MAX_FORMAT_INTEGER};

use super::testing::{first_record, record, renamed_record, GENERATION};
use super::{decode, encode, MIN_SCHEMA, SCHEMA};
use crate::control::testing::{
    array, body_map, field, raw_body, spelled_by_name, with_body_map, with_raw_body,
};
use crate::error::{Error, MalformedDetail};
use crate::generations::generation;
use crate::ControlPayload;

/// A record payload with one field changed by hand, as a reader meets it.
fn tampered(change: impl FnOnce(&mut Vec<(Value, Value)>)) -> ControlPayload {
    tampered_from(&record(), change)
}

/// The same, starting from the record handed in.
fn tampered_from(
    record: &coffret_model::JournalRecord,
    change: impl FnOnce(&mut Vec<(Value, Value)>),
) -> ControlPayload {
    let payload = encode(record).expect("encoding succeeds");
    let mut fields = body_map(&payload, &field_numbers::JOURNAL_RECORD);
    change(&mut fields);
    with_body_map(
        payload.master_key_epoch,
        fields,
        &field_numbers::JOURNAL_RECORD,
    )
}

fn read(payload: &ControlPayload) -> crate::Result<coffret_model::JournalRecord> {
    decode(payload, generation(GENERATION))
}

// FM-15: `additions` is in Container ID order so that one Library state has one
// encoding, and a payload that is not in it is refused rather than sorted.
#[test]
fn additions_out_of_container_id_order_are_rejected() {
    let payload = tampered(|fields| array(fields, "additions").reverse());
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::ControlPayloadOutOfOrder {
                array: "additions",
                index: 1
            })
        ),
        "expected reversed additions to be refused, got {result:?}"
    );
}

// FM-15: the same holds for the removals.
#[test]
fn removals_out_of_container_id_order_are_rejected() {
    let payload = tampered(|fields| array(fields, "removals").reverse());
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::ControlPayloadOutOfOrder {
                array: "removals",
                index: 1
            })
        ),
        "expected reversed removals to be refused, got {result:?}"
    );
}

/// The renamed record's payload with one field of its rename at `index`
/// changed by hand.
fn tampered_rename(index: usize, change: impl FnOnce(&mut Vec<(Value, Value)>)) -> ControlPayload {
    tampered_from(&renamed_record(), |fields| {
        let Value::Map(rename) = &mut array(fields, "renames")[index] else {
            panic!("a rename is a map");
        };
        change(rename);
    })
}

// FM-15: `renames` is ordered by Container ID and then entry number, and a
// payload that is not in it is refused rather than sorted — across Containers
// and within one alike.
#[test]
fn renames_out_of_order_are_rejected() {
    for swap in [(1, 2), (0, 1)] {
        let payload = tampered_from(&renamed_record(), |fields| {
            array(fields, "renames").swap(swap.0, swap.1)
        });
        let result = read(&payload);
        assert!(
            matches!(
                result,
                Err(Error::ControlPayloadOutOfOrder {
                    array: "renames",
                    index: _,
                })
            ),
            "expected renames swapped at {swap:?} to be refused, got {result:?}"
        );
    }
}

// FM-15: `(Container ID, entry number)` names one Entry, so two renames of it
// in one record are refused — by the same strict order.
#[test]
fn a_duplicate_rename_is_rejected() {
    let payload = tampered_rename(1, |rename| *field(rename, "entry") = Value::from(0u64));
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::ControlPayloadOutOfOrder {
                array: "renames",
                index: 1,
            })
        ),
        "expected the second rename of one Entry to be refused, got {result:?}"
    );
}

// FM-15, EP-1, EP-2: a rename's path is a stored Entry Path, so one that is not
// NFC or not in EP-2's shape is malformed rather than repaired.
#[test]
fn a_rename_whose_path_is_not_an_entry_path_is_rejected() {
    let unnormalized = tampered_rename(0, |rename| {
        *field(rename, "path") = Value::Text("albums/cafe\u{301}.jpg".to_owned());
    });
    let result = read(&unnormalized);
    assert!(
        matches!(result, Err(Error::UnnormalizedEntryPath { field: "path" })),
        "expected a decomposed path to be refused, got {result:?}"
    );

    for malformed in ["albums/../etc", "/albums/a.jpg", "albums//a.jpg", ""] {
        let payload = tampered_rename(0, |rename| {
            *field(rename, "path") = Value::Text(malformed.to_owned());
        });
        let result = read(&payload);
        assert!(
            matches!(result, Err(Error::MalformedEntryPath { field: "path" })),
            "expected {malformed:?} to be refused, got {result:?}"
        );
    }
}

// FM-15: derived Entries are not renamed by this version, so no rename gives an
// Entry a path in their reserved namespace.
#[test]
fn a_rename_into_the_derived_namespace_is_rejected() {
    let payload = tampered_rename(2, |rename| {
        *field(rename, "path") = Value::Text(".coffret-derived/thumb/a.jpg".to_owned());
    });
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::RenameIntoReservedNamespace { rename: 2 })
        ),
        "expected a rename into the derived namespace to be refused, got {result:?}"
    );
}

// FM-9: an entry number is a u32, so a rename naming one past it names no
// Entry of any table.
#[test]
fn a_rename_whose_entry_number_is_past_32_bits_is_rejected() {
    let payload = tampered_rename(0, |rename| {
        *field(rename, "entry") = Value::from(u64::from(u32::MAX) + 1);
    });
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::MalformedJournalRecord { ref detail }) if detail.to_string().contains("entry")),
        "expected an entry number past 32 bits to be refused, got {result:?}"
    );
}

// One Container is added once, so a record naming one twice is not a record in
// order with a repeat in it — the same check catches both.
#[test]
fn a_container_added_twice_is_rejected() {
    let payload = tampered(|fields| {
        let additions = array(fields, "additions");
        additions[1] = additions[0].clone();
    });
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::ControlPayloadOutOfOrder {
                array: "additions",
                ..
            })
        ),
        "expected a repeated Container to be refused, got {result:?}"
    );
}

// FM-15: `prev` is the record's own statement of the head it was built on, so
// a record at generation g states g - 1 and nothing else. A reader that took
// the object's name for the chain would replay a record at a position its
// authenticated payload never claimed.
#[test]
fn a_prev_that_is_not_the_previous_generation_is_rejected() {
    let payload = tampered(|fields| *field(fields, "prev") = Value::from(GENERATION - 3));
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::JournalRecordPrevMismatch {
                generation: refused,
                prev,
            }) if refused == generation(GENERATION)
                && prev == Some(generation(GENERATION - 3))
        ),
        "expected a prev naming another head to be refused, got {result:?}"
    );
}

// FM-15: only the record at generation 0 was built on nothing, so a later one
// carrying no `prev` states no head at all.
#[test]
fn a_record_above_generation_zero_without_prev_is_rejected() {
    let payload = tampered(|fields| fields.retain(|(key, _)| key.as_text() != Some("prev")));
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::JournalRecordPrevMismatch {
                generation: refused,
                prev: None
            }) if refused == generation(GENERATION)
        ),
        "expected a record without prev to be refused, got {result:?}"
    );
}

// FM-13: the Library's first head succeeds nothing, so a `prev` on it names a
// head that never existed.
#[test]
fn a_prev_on_the_first_record_is_rejected() {
    let payload = encode(&first_record()).expect("encoding succeeds");
    let mut fields = body_map(&payload, &field_numbers::JOURNAL_RECORD);
    fields.push((Value::Text("prev".to_owned()), Value::from(0u64)));
    let payload = with_body_map(
        payload.master_key_epoch,
        fields,
        &field_numbers::JOURNAL_RECORD,
    );
    let result = decode(&payload, Generation::FIRST);
    assert!(
        matches!(
            result,
            Err(Error::JournalRecordPrevMismatch {
                generation,
                prev: Some(_)
            }) if generation == Generation::FIRST
        ),
        "expected a prev on the first record to be refused, got {result:?}"
    );
}

// FM-15: a reader refuses any schema below the lowest it reads, which is the
// schema before `renames`.
#[test]
fn a_schema_below_the_lowest_readable_one_is_rejected() {
    for schema in [0, MIN_SCHEMA - 1] {
        let payload = tampered(|fields| *field(fields, "schema") = Value::from(schema));
        let result = read(&payload);
        assert!(
            matches!(
                result,
                Err(Error::UnsupportedJournalRecordSchema { schema: found }) if found == schema
            ),
            "expected schema {schema} to be unreadable, got {result:?}"
        );
    }
}

// FM-9, FM-15: a record naming a schema above the one this build implements
// is refused as needing a newer build, whatever else it carries — a reader that
// ignored what it does not know would apply part of it. It is told apart from
// a malformed record, and from one whose own `schema` is too old.
#[test]
fn a_record_requiring_a_newer_reader_is_refused_as_needing_a_newer_build() {
    for record in [record(), renamed_record()] {
        let payload = tampered_from(&record, |fields| {
            fields.retain(|(key, _)| key.as_text() != Some("required_schema"));
            fields.push((
                Value::Text("required_schema".to_owned()),
                Value::from(SCHEMA + 1),
            ));
        });
        let result = read(&payload);
        assert!(
            matches!(
                result,
                Err(Error::JournalRecordNeedsNewerBuild {
                    generation: found,
                    required,
                    implemented: SCHEMA,
                }) if found == generation(GENERATION) && required == SCHEMA + 1
            ),
            "expected the record to need a newer build, got {result:?}"
        );
        let said = result.expect_err("refused").to_string();
        assert!(said.contains("newer build"), "{said}");
    }
}

// FM-15: a record carrying renames without naming the schema they need was
// written by something that does not follow the rule, and is refused rather
// than read as safe for every reader.
#[test]
fn renames_without_a_required_schema_are_rejected() {
    let payload = tampered_from(&renamed_record(), |fields| {
        fields.retain(|(key, _)| key.as_text() != Some("required_schema"))
    });
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::MalformedJournalRecord { ref detail }) if detail.to_string().contains("required_schema")),
        "expected renames without a required schema to be refused, got {result:?}"
    );
}

// FM-15: absent is the reader floor, so naming the floor (or below it) is a
// second spelling of one state and is refused.
#[test]
fn a_required_schema_at_or_below_the_floor_is_rejected() {
    for required in [0, MIN_SCHEMA] {
        let payload = tampered(|fields| {
            fields.push((
                Value::Text("required_schema".to_owned()),
                Value::from(required),
            ))
        });
        let result = read(&payload);
        assert!(
            matches!(result, Err(Error::MalformedJournalRecord { ref detail }) if detail.to_string().contains("required_schema")),
            "expected required_schema {required} to be refused, got {result:?}"
        );
    }
}

#[test]
fn a_missing_field_is_reported_by_name() {
    let payload = tampered(|fields| fields.retain(|(key, _)| key.as_text() != Some("removals")));
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::MalformedJournalRecord { ref detail }) if detail.to_string().contains("removals")),
        "expected the missing field to be named, got {result:?}"
    );
}

#[test]
fn a_field_of_the_wrong_shape_is_reported_by_name() {
    let payload = tampered(|fields| {
        *field(fields, "keyring_set_digest") = Value::from(9u64);
    });
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::MalformedJournalRecord { ref detail })
            if detail.to_string().contains("keyring_set_digest")),
        "expected the field to be named, got {result:?}"
    );
}

// FM-12, KL-3: the digest is the lowercase hex token a Keyring replica's name
// carries, so a payload spelling it otherwise names no replica set this build
// can select against.
#[test]
fn a_keyring_digest_that_is_not_lowercase_hex_is_rejected() {
    let payload = tampered(|fields| {
        *field(fields, "keyring_set_digest") = Value::Text("BEEF".to_owned());
    });
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::Model(coffret_model::Error::InvalidSetDigest { ref digest })) if digest == "BEEF"
        ),
        "expected an uppercase digest to be refused, got {result:?}"
    );
}

// FM-15: a Container ID is sixteen bytes, and a removal that is not is a value
// no Container ever had.
#[test]
fn a_removal_of_the_wrong_length_is_rejected() {
    let payload = tampered(|fields| {
        array(fields, "removals")[0] = Value::Bytes(vec![0x11; 4]);
    });
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::Model(coffret_model::Error::InvalidByteLength {
                expected: 16,
                actual: 4
            }))
        ),
        "expected a four-byte removal to be refused, got {result:?}"
    );
}

#[test]
fn a_removal_that_is_not_a_byte_string_is_rejected() {
    let payload = tampered(|fields| {
        array(fields, "removals")[0] = Value::Text("11111111".to_owned());
    });
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::MalformedJournalRecord { ref detail }) if detail.to_string().contains("removal")),
        "expected a text removal to be refused, got {result:?}"
    );
}

// FM-15: an addition carries an entry table, and each element of it is one
// entry map — the same values FM-9's is, under the catalog's own keys — so an
// element that is not a map at all is refused here as it is there.
#[test]
fn an_entry_that_is_not_an_entry_map_is_rejected() {
    let payload = tampered(|fields| {
        let Value::Map(addition) = &mut array(fields, "additions")[0] else {
            panic!("an addition is a map");
        };
        let Value::Array(entries) = field(addition, "entries") else {
            panic!("an entry table is an array");
        };
        entries[0] = Value::Text("not an entry".to_owned());
    });
    let result = read(&payload);
    let Err(Error::MalformedJournalRecord {
        detail: MalformedDetail::Undecodable(cause),
    }) = result
    else {
        panic!("expected the deserializer to refuse an element that is not a map, got {result:?}");
    };
    // What the deserializer said, not ciborium's `Debug` spelling of it: a
    // cause reading `Custom("…")` would name the layer that caught the value
    // rather than the shape it was expecting there.
    assert!(cause.to_string().contains("expected map"), "{cause}");
    assert!(!cause.to_string().contains("Custom("), "{cause}");
}

// PK-15: `kind` names one of the two kinds a Container can be, so a spelling
// this format version has no kind for is refused rather than guessed at.
#[test]
fn an_addition_of_an_unknown_kind_is_rejected() {
    let payload = tampered(|fields| {
        let Value::Map(addition) = &mut array(fields, "additions")[0] else {
            panic!("an addition is a map");
        };
        *field(addition, "kind") = Value::Text("archive".to_owned());
    });
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::MalformedJournalRecord { ref detail }) if detail.to_string().contains("archive")),
        "expected an unknown kind to be refused, got {result:?}"
    );
}

// FM-11 takes the padding off before this reader sees a body, so bytes after
// the map are bytes no writer following the rule left there.
#[test]
fn bytes_after_the_body_map_are_rejected() {
    let mut payload = encode(&record()).expect("encoding succeeds");
    payload.body.push(0x00);
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::MalformedJournalRecord { ref detail }) if detail.to_string().contains("follow")),
        "expected a trailing byte to be refused, got {result:?}"
    );
}

#[test]
fn a_body_that_is_not_a_map_is_rejected() {
    let payload = encode(&record()).expect("encoding succeeds");
    let mut body = Vec::new();
    ciborium::into_writer(&Value::Text("not a map".to_owned()), &mut body)
        .expect("text serializes");
    let result = read(&ControlPayload::new(payload.master_key_epoch, body));
    assert!(
        matches!(result, Err(Error::MalformedJournalRecord { .. })),
        "expected a body that is not a map to be refused, got {result:?}"
    );
}

#[test]
fn a_malformed_payload_body_names_the_fault_without_the_wrapping() {
    let payload = encode(&record()).expect("encoding succeeds");
    // An unassigned CBOR simple value: well formed enough for ciborium to read
    // a header off, and then not an item it can make a value of — which is the
    // reading that carries a message.
    let body = vec![0xe0];
    let result = read(&ControlPayload::new(payload.master_key_epoch, body));
    let Err(Error::MalformedJournalRecord {
        detail: MalformedDetail::Undecodable(cause),
    }) = result
    else {
        panic!("expected the decoder to refuse an unreadable body, got {result:?}");
    };
    // What ciborium said, not its `Debug` spelling of it: a cause reading
    // `Semantic(None, "…")` would name the layer that caught the bytes rather
    // than the fault it found in them.
    assert!(
        cause.to_string().contains("known simple value"),
        "the cause does not say what ciborium refused: {cause}"
    );
    assert!(
        !cause.to_string().contains("Semantic("),
        "the cause carries ciborium's wrapper around its message: {cause}"
    );
}

// EP-2: an entry table in a record carries paths the Library already holds, so
// one outside the shape every Entry Path is in was written by something that did
// not hold to EP-2 — the record does not decode, and the field that carried it
// is named.
#[test]
fn an_entry_path_with_a_shape_ep_2_excludes_is_rejected() {
    let payload = tampered(|fields| {
        let Value::Map(addition) = &mut array(fields, "additions")[0] else {
            panic!("an addition is a CBOR map");
        };
        let Value::Array(entries) = field(addition, "entries") else {
            panic!("an entry table is a CBOR array");
        };
        let Value::Map(entry) = &mut entries[0] else {
            panic!("an entry is a CBOR map");
        };
        *field(entry, "path") = Value::Text("../x".to_owned());
    });
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::MalformedEntryPath { field: "path" })),
        "expected a `..` component to be refused, got {result:?}"
    );
}

// EP-2, FM-9: an Entry's captured path is the name a Container's meta section
// records, and FM-9 holds that name to EP-1 as well, so one outside the shape
// is refused in the same way, naming the field that carried it.
#[test]
fn a_captured_path_with_a_shape_ep_2_excludes_is_rejected() {
    let payload = tampered(|fields| {
        *entry_field(fields, "captured_path") = Value::Text("../x".to_owned());
    });
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::MalformedEntryPath {
                field: "captured_path"
            })
        ),
        "expected a `..` component to be refused, got {result:?}"
    );
}

// FM-15, FM-16: `captured_path` is written only where it differs from `path`,
// so one state has one encoding. A map carrying it equal to `path` is a second
// spelling of a map without it, and is refused rather than accepted as one.
#[test]
fn a_captured_path_equal_to_the_path_is_rejected() {
    let payload = tampered(|fields| {
        let path = entry_field(fields, "path").clone();
        let Value::Map(entry) = entry_map(fields) else {
            panic!("an entry is a CBOR map");
        };
        entry.retain(|(key, _)| key.as_text() != Some("captured_path"));
        entry.push((Value::Text("captured_path".to_owned()), path));
    });
    let result = read(&payload);
    let Err(Error::MalformedJournalRecord {
        detail: MalformedDetail::Written(detail),
    }) = result
    else {
        panic!("expected a second spelling of one state to be refused, got {result:?}");
    };
    assert!(detail.contains("captured_path"), "{detail}");
}

// FM-9, FM-15, FM-19: an addition carries the entry table of the Container it
// adds, and each row of it places an Entry against a plaintext stream whose
// positions the format bounds. A row whose `offset` and `size` end past the last
// of them places nothing, so the record does not decode — the same refusal a
// meta section carrying such a row gets, because it is the same table.
#[test]
fn an_entry_extent_past_the_end_of_the_address_space_is_rejected() {
    let payload = tampered(|fields| {
        *entry_field(fields, "offset") = Value::from(MAX_FORMAT_INTEGER);
    });
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::StreamTooLong)),
        "expected an extent running past the address space to be refused, got {result:?}"
    );
}

// FM-19: an entry map's own numbers are integers the format bounds like any
// other, so one at the bound is a malformed record rather than a table that
// merely runs off the end — the object is refused either way, and the reason
// given is the one that applies.
//
// The detail names the field and the number, as every other malformed field of
// these schemas does: both are the format's own arithmetic and neither says
// anything about the Library's content.
#[test]
fn an_entry_integer_past_the_formats_integer_range_is_malformed() {
    let past_the_bound = MAX_FORMAT_INTEGER + 1;
    let payload = tampered(|fields| {
        *entry_field(fields, "offset") = Value::from(past_the_bound);
    });
    let result = read(&payload);
    let Err(Error::MalformedJournalRecord {
        detail: MalformedDetail::Written(detail),
    }) = result
    else {
        panic!("expected an offset of 2^63 to be malformed, got {result:?}");
    };
    assert!(detail.contains("offset"), "{detail}");
    assert!(detail.contains("below 2^63"), "{detail}");
    assert!(detail.contains(&past_the_bound.to_string()), "{detail}");
}

/// The first entry map of the first addition, whole.
fn entry_map(fields: &mut [(Value, Value)]) -> &mut Value {
    let Value::Map(addition) = &mut array(fields, "additions")[0] else {
        panic!("an addition is a CBOR map");
    };
    let Value::Array(entries) = field(addition, "entries") else {
        panic!("an entry table is a CBOR array");
    };
    &mut entries[0]
}

/// The value one key of the first entry of the first addition holds.
fn entry_field<'a>(fields: &'a mut [(Value, Value)], key: &str) -> &'a mut Value {
    let Value::Map(addition) = &mut array(fields, "additions")[0] else {
        panic!("an addition is a CBOR map");
    };
    let Value::Array(entries) = field(addition, "entries") else {
        panic!("an entry table is a CBOR array");
    };
    let Value::Map(entry) = &mut entries[0] else {
        panic!("an entry is a CBOR map");
    };
    field(entry, key)
}

// FM-10: a Container is built out of Entries, so an addition whose table holds
// none describes a Container no writer produces — and a record carrying one
// would add a Container to every Index without adding a single Entry. The
// meta-section reader has always refused it; the record's reader refuses it
// now, because the rule belongs to the addition rather than to either reader.
#[test]
fn an_addition_with_no_entries_is_rejected() {
    let payload = tampered(|fields| {
        let Value::Map(addition) = &mut array(fields, "additions")[0] else {
            panic!("an addition is a CBOR map");
        };
        *field(addition, "entries") = Value::Array(Vec::new());
    });
    let result = read(&payload);
    assert!(
        matches!(result, Err(Error::AdditionWithoutEntries { addition: 0 })),
        "expected an addition with an empty table to be refused, got {result:?}"
    );
}

// FM-9: the entry table tiles the Container's plaintext stream, so every Entry
// begins where its predecessor ended. A record whose table leaves a gap would
// apply to an Index that then answers a range read with bytes belonging to
// nothing.
#[test]
fn an_addition_whose_entries_do_not_tile_is_rejected() {
    let payload = tampered(|fields| {
        // The addition at 1 is the Pack, whose table carries two Entries.
        let Value::Map(addition) = &mut array(fields, "additions")[1] else {
            panic!("an addition is a CBOR map");
        };
        let Value::Array(entries) = field(addition, "entries") else {
            panic!("an entry table is a CBOR array");
        };
        let Value::Map(entry) = &mut entries[1] else {
            panic!("an entry is a CBOR map");
        };
        *field(entry, "offset") = Value::from(130u64);
    });
    let result = read(&payload);
    assert!(
        matches!(
            result,
            Err(Error::AdditionEntriesDoNotTile {
                addition: 1,
                entry: 1,
                expected: 120,
                found: 130,
            })
        ),
        "expected a gap in an entry table to be refused, got {result:?}"
    );
}

// FM-15: every field travels under the field number the registry assigns it,
// and a text key is no field's spelling, so a map that carries its fields only
// under their names is missing them — refused by the name of the first one the
// reader asks for.
#[test]
fn a_record_whose_fields_are_keyed_by_name_is_missing_them() {
    let payload = spelled_by_name(
        &encode(&record()).expect("encoding succeeds"),
        &field_numbers::JOURNAL_RECORD,
    );
    let result = read(&payload);
    let Err(Error::MalformedJournalRecord { detail }) = result else {
        panic!("expected a record keyed by name to be refused, got {result:?}");
    };
    assert!(detail.to_string().contains("schema is missing"), "{detail}");
}

// FM-15: the same holds a level down: an entry whose `path` is carried under
// its name rather than its field number has no `path`.
#[test]
fn an_entry_whose_path_is_keyed_by_name_is_missing_it() {
    let payload = encode(&record()).expect("encoding succeeds");
    let mut fields = raw_body(&payload);
    let additions_key = Value::from(
        field_numbers::JOURNAL_RECORD
            .number_of("additions")
            .unwrap(),
    );
    let entries_key = Value::from(field_numbers::ADDITION.number_of("entries").unwrap());
    let path_key = Value::from(field_numbers::ADDITION_ENTRY.number_of("path").unwrap());
    let (_, Value::Array(additions)) = fields
        .iter_mut()
        .find(|(key, _)| *key == additions_key)
        .expect("the record carries additions")
    else {
        panic!("additions is an array");
    };
    let Value::Map(addition) = &mut additions[0] else {
        panic!("an addition is a map");
    };
    let (_, Value::Array(entries)) = addition
        .iter_mut()
        .find(|(key, _)| *key == entries_key)
        .expect("an addition carries its entry table")
    else {
        panic!("an entry table is an array");
    };
    let Value::Map(entry) = &mut entries[0] else {
        panic!("an entry is a map");
    };
    for (key, _) in entry.iter_mut() {
        if *key == path_key {
            *key = Value::Text("path".to_owned());
        }
    }
    let result = read(&with_raw_body(payload.master_key_epoch, Value::Map(fields)));
    let Err(Error::MalformedJournalRecord { detail }) = result else {
        panic!("expected an entry keyed by name to be refused, got {result:?}");
    };
    assert!(detail.to_string().contains("`path`"), "{detail}");
}
