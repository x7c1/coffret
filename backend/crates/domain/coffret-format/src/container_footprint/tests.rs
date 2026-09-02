use coffret_model::{ContainerKey, ContainerKind, ContentHash, EntryPath, Mtime};

use super::*;
use crate::container_writer::ContainerWriter;
use crate::encode_plan::EncodePlan;
use crate::meta::{self, Meta};

fn plan(path: &str, size: u64) -> EntryPlan {
    EntryPlan::new(
        EntryPath::nfc(path.to_owned()),
        Mtime::from_unix_seconds(1_700_000_000),
        size,
        ContentHash::from_bytes([0x11; ContentHash::BYTE_LEN]),
    )
}

/// The same measurement taken the slow way: lay the entry table out and count
/// what it serializes to.
fn measured(kind: ContainerKind, entries: &[EntryPlan]) -> u64 {
    let mut offset = 0u64;
    let table: Vec<_> = entries
        .iter()
        .map(|entry| {
            let metadata = entry.to_metadata(offset);
            offset += entry.size;
            metadata
        })
        .collect();
    let content = offset;
    let map = meta::encode(&Meta {
        kind,
        pad_len: padme::padded_len(content) - content,
        entries: table,
    })
    .expect("a table this size encodes");
    Header::LEN as u64 + map.len() as u64 + content
}

/// What a header would declare for this table's meta section, laid out the slow
/// way: the CBOR map carried to its Padmé bucket, with the tag on it.
fn measured_meta_len(kind: ContainerKind, entries: &[EntryPlan]) -> u64 {
    let mut offset = 0u64;
    let table: Vec<_> = entries
        .iter()
        .map(|entry| {
            let metadata = entry.to_metadata(offset);
            offset += entry.size;
            metadata
        })
        .collect();
    let content = offset;
    let map = meta::encode(&Meta {
        kind,
        pad_len: padme::padded_len(content) - content,
        entries: table,
    })
    .expect("a table this size encodes");
    padme::padded_len(map.len() as u64) + crate::aead::TAG_LEN as u64
}

// PK-6: the target applies to Entry contents, canonical metadata, and framing.
// An accumulator that drifted from what the encoder actually lays out would
// make the target mean one thing to the policy and another on Storage.
#[test]
fn the_accumulated_footprint_is_the_laid_out_one() {
    let tables: Vec<Vec<EntryPlan>> = vec![
        vec![plan("a.jpg", 0)],
        vec![plan("albums/2026/spring.jpg", 4096)],
        (0..30)
            .map(|index| plan(&format!("albums/2026/{index:04}.jpg"), 1_000 + index))
            .collect(),
        // Past the point where the array header and the offsets widen.
        (0..300)
            .map(|index| plan(&format!("books/atlas/page-{index:05}.png"), 700_000))
            .collect(),
    ];

    for entries in tables {
        for kind in [ContainerKind::OneFile, ContainerKind::Pack] {
            let footprint = ContainerFootprint::of(kind, &entries).expect("a table this size");
            assert_eq!(
                footprint.bytes(),
                measured(kind, &entries),
                "{} entries of kind {kind:?}",
                entries.len(),
            );
            assert_eq!(footprint.entries(), entries.len());
            // FM-2: and the same for the one number the header declares, which
            // is what segmentation closes a Pack on and what a reader holds
            // against `Header::MAX_META_LEN`. An accumulator that drifted here
            // would let a writer lay out a Container no reader would open.
            assert_eq!(
                footprint.meta_len(),
                measured_meta_len(kind, &entries),
                "the declared meta length of {} entries of kind {kind:?}",
                entries.len(),
            );
        }
    }
}

// PK-6: authentication tags and Padmé padding come after the measurement, so
// the stored object is the footprint or larger — never smaller, which would
// make the target unenforceable in the direction it matters.
#[test]
fn the_stored_object_is_never_smaller_than_the_footprint() {
    let entries: Vec<EntryPlan> = (0..12)
        .map(|index| plan(&format!("albums/{index:02}.jpg"), 400 + index * 37))
        .collect();
    let footprint =
        ContainerFootprint::of(ContainerKind::Pack, &entries).expect("a table this size");

    // Content whose hash is the one the plans declare, so the writer accepts it.
    let entries: Vec<EntryPlan> = entries
        .into_iter()
        .map(|mut entry| {
            let content = vec![0x5b; entry.size as usize];
            entry.hash = ContentHash::from_bytes(*blake3::hash(&content).as_bytes());
            entry
        })
        .collect();

    let key = ContainerKey::from_bytes([0x3c; ContainerKey::BYTE_LEN]);
    let encode_plan = EncodePlan::new(
        crate::generate_container_id().expect("the OS CSPRNG is available"),
        ContainerKind::Pack,
        &key,
        &entries,
    );
    let mut object = Vec::new();
    let mut writer =
        ContainerWriter::begin(&encode_plan, &mut object).expect("the plan is written");
    for entry in &entries {
        writer
            .write(&vec![0x5b; entry.size as usize], &mut object)
            .expect("the content is fed");
    }
    writer.finish(&mut object).expect("the Container closes");

    assert!(
        object.len() as u64 >= footprint.bytes(),
        "a {}-byte object measured {} bytes before padding",
        object.len(),
        footprint.bytes(),
    );
}

// PK-3 needs the measurement to grow with every Entry, or a segmentation could
// append forever without the target ever being reached.
#[test]
fn every_appended_entry_costs_something() {
    let mut footprint = ContainerFootprint::empty(ContainerKind::Pack).expect("an empty Pack");
    for index in 0..50 {
        let entry = plan(&format!("a/{index:03}"), 0);
        let extended = footprint.extended(&entry).expect("one more Entry");
        assert!(
            extended.bytes() > footprint.bytes(),
            "an Entry with no content still costs its table row",
        );
        assert_eq!(extended.content_bytes(), footprint.content_bytes());
        footprint = extended;
    }
}
