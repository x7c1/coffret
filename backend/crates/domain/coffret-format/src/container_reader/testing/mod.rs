//! Helpers shared by the range reader's tests.
//!
//! Every case is built the same way: encode a Pack of several Entries with a
//! chunk size small enough that the cut falls between them, then ask for one
//! Entry and check that what comes back is that Entry's bytes and that no more
//! of the object than the chunks covering it was ever touched.

use coffret_model::{ContainerId, ContainerKey, ContainerKind, EntryMetadata, Mtime};

use crate::chunk_size::ChunkSize;
use crate::container_reader::{ChunkRunReader, ContainerOutline};
use crate::encode::encode;
use crate::encode_request::EncodeRequest;
use crate::entry_paths::entry_path;
use crate::entry_source::EntrySource;
use crate::header::Header;

/// A chunk size small enough that a handful of short files spans several
/// chunks.
pub(super) const SMALL_CHUNK: u32 = 16;

pub(super) fn key() -> ContainerKey {
    ContainerKey::from_bytes([0x11; ContainerKey::BYTE_LEN])
}

pub(super) fn container_id() -> ContainerId {
    ContainerId::from_bytes([0x22; ContainerId::BYTE_LEN])
}

/// Content that differs in every byte, so bytes taken from the wrong offset
/// land on different values rather than on the same ones.
pub(super) fn filler(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|index| (index as u8).wrapping_mul(31).wrapping_add(seed))
        .collect()
}

/// A Pack of the given contents, at a chunk size a case can reason about.
pub(super) fn pack(contents: &[Vec<u8>]) -> Vec<u8> {
    let entries: Vec<EntrySource<'_>> = contents
        .iter()
        .enumerate()
        .map(|(index, content)| {
            EntrySource::new(
                entry_path(format!("books/atlas/{index:03}.jpg")),
                Mtime::from_unix_seconds(1_700_000_000),
                content,
            )
        })
        .collect();
    encode(&EncodeRequest {
        container_id: container_id(),
        kind: ContainerKind::Pack,
        key: &key(),
        chunk_size: ChunkSize::new(SMALL_CHUNK).expect("the chunk size is non-zero"),
        entries: &entries,
    })
    .expect("encoding a Pack of these entries succeeds")
    .into_bytes()
}

/// The outline of an object, read the way a range reader reads it: the header
/// first, then exactly as many more bytes as the header asks for.
pub(super) fn outline_of(object: &[u8]) -> ContainerOutline {
    let prefix_len = ContainerOutline::prefix_len(&object[..Header::LEN])
        .expect("a well-formed header says how long its meta section is")
        as usize;
    ContainerOutline::open(&object[..prefix_len], &key()).expect("the meta section opens")
}

/// Reads one Entry the way a partial fetch does, returning its plaintext and
/// how many ciphertext bytes were asked for.
pub(super) fn read_entry(object: &[u8], path: &str) -> (Vec<u8>, u64) {
    let outline = outline_of(object);
    let entry = outline
        .entry_at(&entry_path(path))
        .expect("the Pack holds the Entry the case asked for")
        .clone();
    let run = outline
        .chunks_covering(entry.offset..entry.offset + entry.size)
        .expect("an Entry's own extent lies inside its Container's stream");
    let asked = run.ciphertext();

    let mut plaintext = Vec::new();
    let mut reader = ChunkRunReader::begin(&outline, &key(), &run);
    reader
        .read(
            &object[asked.start as usize..asked.end as usize],
            &mut plaintext,
        )
        .expect("the chunks covering an Entry open");
    reader.finish().expect("the whole run arrived");

    let start = (entry.offset - run.plaintext_start()) as usize;
    (
        plaintext[start..start + entry.size as usize].to_vec(),
        asked.end - asked.start,
    )
}

/// Where one Entry stands in a Pack's plaintext stream.
pub(super) fn entry_of(object: &[u8], path: &str) -> EntryMetadata {
    outline_of(object)
        .entry_at(&entry_path(path))
        .expect("the Pack holds the Entry the case asked for")
        .clone()
}
