//! What survives a trip through `encode` and back.

use coffret_model::{ContainerId, ContainerKind, DerivedFrom, EntryPath, Mtime};

use super::decode;
use super::testing::{chunk_ranges, container_id, encode_with, key, source, SMALL_CHUNK};
use crate::aead::TAG_LEN;
use crate::chunk_size::ChunkSize;
use crate::encode::encode;
use crate::encode_request::EncodeRequest;
use crate::header::Header;
use crate::padme;

// FM-2, FM-5, FM-9: a multi-entry Container round-trips — every Entry comes
// back byte-identical, with the metadata and the Container kind it went in
// with.
#[test]
fn multi_entry_container_round_trips() {
    let first = vec![0xa1u8; 40];
    let second = b"the second entry".to_vec();
    let third = vec![0u8; 0];
    let entries = [
        source("photos/one.jpg", &first),
        {
            let mut entry = source("notes/two.txt", &second);
            entry.mime = Some("text/plain".to_owned());
            entry.derived_from = Some(DerivedFrom {
                container_id: ContainerId::from_bytes([0x33; ContainerId::BYTE_LEN]),
                path: EntryPath::new("originals/two.txt"),
            });
            entry
        },
        source("empty", &third),
    ];
    let encoded = encode_with(SMALL_CHUNK, &entries);
    let decoded = decode(encoded.bytes(), &key()).expect("the object is intact");

    assert_eq!(decoded.container_id, container_id());
    assert_eq!(decoded.kind, ContainerKind::Pack);
    assert_eq!(decoded.entries.len(), 3);

    let contents: Vec<&[u8]> = decoded
        .entries
        .iter()
        .map(|entry| entry.content.as_slice())
        .collect();
    assert_eq!(contents, vec![first.as_slice(), second.as_slice(), &[][..]]);

    let paths: Vec<&str> = decoded
        .entries
        .iter()
        .map(|entry| entry.metadata.path.as_str())
        .collect();
    assert_eq!(paths, ["photos/one.jpg", "notes/two.txt", "empty"]);

    for entry in &decoded.entries {
        assert_eq!(
            entry.metadata.mtime,
            Mtime::from_unix_seconds(1_700_000_000)
        );
        assert_eq!(
            entry.metadata.hash.as_bytes(),
            blake3::hash(&entry.content).as_bytes()
        );
        assert_eq!(entry.metadata.size, entry.content.len() as u64);
    }
    assert_eq!(decoded.entries[0].metadata.offset, 0);
    assert_eq!(decoded.entries[1].metadata.offset, 40);
    assert_eq!(decoded.entries[2].metadata.offset, 56);
    assert_eq!(
        decoded.entries[1].metadata.mime.as_deref(),
        Some("text/plain")
    );
    assert_eq!(
        decoded.entries[1].metadata.derived_from,
        Some(DerivedFrom {
            container_id: ContainerId::from_bytes([0x33; ContainerId::BYTE_LEN]),
            path: EntryPath::new("originals/two.txt"),
        })
    );
}

// FM-3: the Storage object name is the Container ID as 32 lowercase hex
// characters followed by `.cfrt`.
#[test]
fn object_name_is_the_container_id_in_hex() {
    let content = b"x".to_vec();
    let encoded = encode_with(SMALL_CHUNK, &[source("a", &content)]);
    assert_eq!(
        encoded.object_name(),
        "22222222222222222222222222222222.cfrt"
    );
}

// FM-4: the plaintext stream is padded up to the next Padmé bucket, and the
// meta section's `pad_len` records exactly that padding — so decoding
// strips it and returns the Entry's own bytes.
#[test]
fn padding_is_recorded_and_stripped() {
    let content = vec![0x77u8; 9];
    let encoded = encode_with(SMALL_CHUNK, &[source("a", &content)]);
    let object = encoded.bytes();

    // 9 bytes pad up to the bucket boundary at 10.
    assert_eq!(padme::padded_len(9), 10);
    let header = Header::parse(object).expect("the object has a valid header");
    let padded_stream_len = object.len() - Header::LEN - header.meta_len as usize - TAG_LEN;
    assert_eq!(padded_stream_len, 10);

    let decoded = decode(object, &key()).expect("the object is intact");
    assert_eq!(decoded.entries[0].content, content);
    assert_eq!(decoded.entries[0].metadata.size, 9);
}

// FM-4, FM-5: a Container whose Entries are all zero-byte files still has a
// plaintext stream — an empty one, needing no padding — and it is cut into
// one empty final chunk, so the object still ends with a final-chunk
// message and still round-trips.
#[test]
fn container_of_only_empty_entries_round_trips() {
    let entries = [source("a", b""), source("b", b"")];
    let encoded = encode_with(SMALL_CHUNK, &entries);
    let header = Header::parse(encoded.bytes()).expect("the object has a valid header");
    assert_eq!(
        encoded.bytes().len(),
        Header::LEN + header.meta_len as usize + TAG_LEN,
        "the single chunk message is a tag and nothing else"
    );

    let decoded = decode(encoded.bytes(), &key()).expect("the object is intact");
    assert_eq!(decoded.entries.len(), 2);
    for entry in &decoded.entries {
        assert!(entry.content.is_empty());
        assert_eq!(entry.metadata.size, 0);
        assert_eq!(entry.metadata.offset, 0);
    }
}

// FM-5, FM-7: when the padded stream is an exact multiple of the chunk size
// the last chunk carries a full chunk of plaintext rather than a short
// remainder, and it still has to be read under the final-chunk domain. Padmé
// makes this the ordinary case rather than an edge one: from 32 MiB up the
// bucket is a multiple of the 1 MiB default chunk size, so every large
// Container ends this way.
#[test]
fn stream_ending_on_a_chunk_boundary_round_trips() {
    let content = vec![0x6bu8; 3 * SMALL_CHUNK as usize];
    // The length is already a bucket boundary, so the stream is unpadded and
    // divides into whole chunks with nothing left over.
    assert_eq!(
        padme::padded_len(content.len() as u64),
        content.len() as u64
    );

    let encoded = encode_with(SMALL_CHUNK, &[source("a", &content)]);
    let ranges = chunk_ranges(encoded.bytes(), SMALL_CHUNK);
    assert_eq!(ranges.len(), 3);
    assert_eq!(
        ranges[2].len(),
        SMALL_CHUNK as usize + TAG_LEN,
        "the final chunk is a full one"
    );

    let decoded = decode(encoded.bytes(), &key()).expect("the object is intact");
    assert_eq!(decoded.entries[0].content, content);
}

// FM-6: the chunk size is a per-Container parameter, and a reader honors
// the value recorded in the header rather than assuming the default.
#[test]
fn non_default_chunk_size_round_trips() {
    let content = vec![0x3cu8; 200];
    let entries = [source("a", &content)];
    let encoded = encode_with(24, &entries);
    assert_ne!(24, ChunkSize::DEFAULT.get());

    let header = Header::parse(encoded.bytes()).expect("the object has a valid header");
    assert_eq!(header.chunk_size.get(), 24);
    assert_eq!(chunk_ranges(encoded.bytes(), 24).len(), 9);

    let decoded = decode(encoded.bytes(), &key()).expect("the object is intact");
    assert_eq!(decoded.chunk_size.get(), 24);
    assert_eq!(decoded.entries[0].content, content);
}

#[test]
fn default_chunk_size_round_trips() {
    let content = vec![0x3cu8; 3 * 1024 * 1024 + 7];
    let entries = [source("a", &content)];
    let container_key = key();
    let request = EncodeRequest::new(
        container_id(),
        ContainerKind::OneFile,
        &container_key,
        &entries,
    );
    let encoded = encode(&request).expect("encoding succeeds");

    let decoded = decode(encoded.bytes(), &container_key).expect("the object is intact");
    assert_eq!(decoded.chunk_size, ChunkSize::DEFAULT);
    assert_eq!(decoded.kind, ContainerKind::OneFile);
    assert_eq!(decoded.entries[0].content, content);
}
