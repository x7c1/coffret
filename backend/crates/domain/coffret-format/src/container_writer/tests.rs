use coffret_model::{ContainerId, ContainerKey, ContainerKind, ContentHash, Mtime};

use super::*;
use crate::chunk_size::ChunkSize;
use crate::decode::decode;
use crate::encode::encode;
use crate::encode_request::EncodeRequest;
use crate::entry_paths::entry_path;
use crate::entry_plan::EntryPlan;
use crate::entry_source::EntrySource;

fn key() -> ContainerKey {
    ContainerKey::from_bytes([0x3c; ContainerKey::BYTE_LEN])
}

fn container_id() -> ContainerId {
    ContainerId::from_bytes([0x5e; ContainerId::BYTE_LEN])
}

/// Content that differs in every byte, so a reader that dropped or reordered
/// bytes lands somewhere else rather than back where it started.
fn filler(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|index| (index as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

fn source<'a>(path: &str, content: &'a [u8]) -> EntrySource<'a> {
    EntrySource::new(
        entry_path(path.to_owned()),
        Mtime::from_unix_seconds(1_700_000_000),
        content,
    )
}

fn plan_of(entry: &EntrySource<'_>) -> EntryPlan {
    EntryPlan::new(
        entry.path.clone(),
        entry.mtime,
        entry.content.len() as u64,
        ContentHash::from_bytes(*blake3::hash(entry.content).as_bytes()),
    )
}

/// Writes a Container the streaming way, feeding the content in `pieces`-byte
/// bites so that a chunk boundary and a caller's buffer boundary do not line up.
fn streamed(
    kind: ContainerKind,
    chunk_size: ChunkSize,
    entries: &[EntrySource<'_>],
    pieces: usize,
) -> Vec<u8> {
    let plans: Vec<EntryPlan> = entries.iter().map(plan_of).collect();
    let key = key();
    let plan = EncodePlan {
        container_id: container_id(),
        kind,
        key: &key,
        chunk_size,
        entries: &plans,
    };

    let mut object = Vec::new();
    let mut writer = ContainerWriter::begin(&plan, &mut object).expect("the plan is writable");
    for entry in entries {
        for piece in entry.content.chunks(pieces.max(1)) {
            writer.write(piece, &mut object).expect("a piece is fed");
        }
    }
    writer.finish(&mut object).expect("the Container closes");
    object
}

fn encoded(kind: ContainerKind, chunk_size: ChunkSize, entries: &[EntrySource<'_>]) -> Vec<u8> {
    let key = key();
    encode(&EncodeRequest {
        container_id: container_id(),
        kind,
        key: &key,
        chunk_size,
        entries,
    })
    .expect("the request is encodable")
    .into_bytes()
}

// The property the whole streaming path rests on: for one entry table and one
// stream, the writer lays down the object `encode` would have. If it did not,
// a Pack would be a second dialect of the format nothing else reads
// (spec: FM-1, FM-2, FM-3, FM-4, FM-5, FM-6, FM-7, FM-8, FM-9).
#[test]
fn the_streamed_object_is_the_encoded_object() {
    let first = filler(100, 0x11);
    let second = filler(4096, 0x37);
    let third = filler(7, 0x5b);
    let large = filler(9000, 0x2d);

    let cases: Vec<(&str, ContainerKind, ChunkSize, Vec<EntrySource<'_>>, usize)> = vec![
        (
            "one entry, one chunk",
            ContainerKind::OneFile,
            ChunkSize::DEFAULT,
            vec![source("photos/spring.jpg", &first)],
            17,
        ),
        (
            "a Pack whose stream spans several chunks",
            ContainerKind::Pack,
            ChunkSize::new(64).expect("64 is a chunk size"),
            vec![
                source("album/2019/party.jpg", &first),
                source("notes/ancient.txt", &second),
                source("photos/thumb.webp", &third),
            ],
            13,
        ),
        (
            "a stream that ends exactly on a chunk boundary",
            ContainerKind::Pack,
            ChunkSize::new(1000).expect("1000 is a chunk size"),
            vec![source("a", &second)],
            256,
        ),
        (
            "entries that are all empty",
            ContainerKind::Pack,
            ChunkSize::DEFAULT,
            vec![source("empty/first", b""), source("empty/second", b"")],
            8,
        ),
        (
            "an empty Entry between two that are not",
            ContainerKind::Pack,
            ChunkSize::new(32).expect("32 is a chunk size"),
            vec![
                source("a", &third),
                source("b", b""),
                source("c", &first),
                source("d", b""),
            ],
            5,
        ),
        (
            "one Entry larger than the target, fed in one bite",
            ContainerKind::Pack,
            ChunkSize::new(1024).expect("1024 is a chunk size"),
            vec![source("raw/huge.arw", &large)],
            usize::MAX,
        ),
    ];

    for (what, kind, chunk_size, entries, pieces) in cases {
        assert_eq!(
            streamed(kind, chunk_size, &entries, pieces),
            encoded(kind, chunk_size, &entries),
            "{what}",
        );
    }
}

// FM-5, FM-7: what the writer produces is a Container, chunk domains and all,
// and not merely a byte string that happens to match.
#[test]
fn a_streamed_pack_decodes_back_to_its_entries() {
    let first = filler(300, 0x11);
    let second = filler(120, 0x37);
    let entries = [
        source("album/a.jpg", &first),
        source("album/b.jpg", &second),
    ];

    let object = streamed(
        ContainerKind::Pack,
        ChunkSize::new(64).expect("64 is a chunk size"),
        &entries,
        11,
    );
    let opened = decode(&object, &key()).expect("a streamed Pack decodes");

    assert_eq!(opened.kind, ContainerKind::Pack);
    assert_eq!(opened.entries.len(), 2);
    assert_eq!(opened.entries[0].content, first);
    assert_eq!(opened.entries[1].content, second);
    assert_eq!(opened.entries[1].metadata.offset, first.len() as u64);
}

// The entry table is written before the content arrives, so the writer has to
// be the one that refuses content the table does not describe — otherwise a
// file that changed under a run reaches Storage inside a Container whose table
// is a lie about it.
#[test]
fn content_that_is_not_what_the_plan_declares_is_refused() {
    let content = filler(64, 0x11);
    let plans = [plan_of(&source("a.jpg", &content))];
    let key = key();
    let plan = EncodePlan::new(container_id(), ContainerKind::Pack, &key, &plans);

    let mut object = Vec::new();
    let mut writer = ContainerWriter::begin(&plan, &mut object).expect("the plan is writable");
    let mut wrong = content.clone();
    wrong[0] ^= 0x01;
    writer
        .write(&wrong, &mut object)
        .expect("the bytes are fed");
    let result = writer.finish(&mut object);
    assert!(
        matches!(result, Err(Error::EntryHashMismatch { index: 0 })),
        "expected the hash the plan declares to be held to, got {result:?}",
    );

    let mut object = Vec::new();
    let mut writer = ContainerWriter::begin(&plan, &mut object).expect("the plan is writable");
    writer
        .write(&content[..10], &mut object)
        .expect("the bytes are fed");
    let result = writer.finish(&mut object);
    assert!(
        matches!(
            result,
            Err(Error::EntryLengthMismatch {
                index: 0,
                expected: 64,
                actual: 10
            })
        ),
        "expected a short Entry to be refused, got {result:?}",
    );

    let mut object = Vec::new();
    let mut writer = ContainerWriter::begin(&plan, &mut object).expect("the plan is writable");
    writer
        .write(&content, &mut object)
        .expect("the bytes are fed");
    let result = writer.write(b"one byte too many", &mut object);
    assert!(
        matches!(result, Err(Error::StreamOverrun { planned: 64 })),
        "expected bytes past the entry table to be refused, got {result:?}",
    );
}

// FM-10: a Container exists to hold user data, so there is no empty one to write
// — the streaming path refuses it where `encode` does, and before any object
// bytes reach the caller's buffer.
#[test]
fn an_empty_entry_table_is_refused() {
    let key = key();
    let plan = EncodePlan::new(container_id(), ContainerKind::Pack, &key, &[]);
    let mut object = Vec::new();
    let result = ContainerWriter::begin(&plan, &mut object);
    assert!(
        matches!(result, Err(Error::EmptyEntryTable)),
        "expected an empty entry table to be refused, got {:?}",
        result.map(|_| ()),
    );
    assert!(object.is_empty(), "nothing was written for a refused plan");
}

// The claim the freeze spool rests on: a Container far larger than the buffer
// the caller drains still costs the caller one chunk of ciphertext at a time.
#[test]
fn the_caller_never_has_to_hold_more_than_a_chunk() {
    const CHUNK: usize = 512;
    const SIZE: usize = 40 * CHUNK;

    let content = filler(SIZE, 0x2d);
    let plans = [plan_of(&source("raw/huge.arw", &content))];
    let key = key();
    let plan = EncodePlan {
        container_id: container_id(),
        kind: ContainerKind::Pack,
        key: &key,
        chunk_size: ChunkSize::new(CHUNK as u32).expect("a chunk size"),
        entries: &plans,
    };

    let mut out = Vec::new();
    let mut object = Vec::new();
    let mut writer = ContainerWriter::begin(&plan, &mut out).expect("the plan is writable");
    let mut high_water = out.len();
    object.append(&mut out);

    for piece in content.chunks(64) {
        writer.write(piece, &mut out).expect("a piece is fed");
        high_water = high_water.max(out.len());
        object.append(&mut out);
    }
    writer.finish(&mut out).expect("the Container closes");
    high_water = high_water.max(out.len());
    object.append(&mut out);

    assert!(
        high_water < 4 * CHUNK,
        "a drained sink never held more than a chunk or so, but reached {high_water}",
    );
    assert_eq!(
        object,
        encoded(
            ContainerKind::Pack,
            ChunkSize::new(CHUNK as u32).expect("a chunk size"),
            &[source("raw/huge.arw", &content)],
        ),
        "and the object is still the one `encode` writes",
    );
}
