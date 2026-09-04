//! What a Container's own header may cost a reader before it is believed.
//!
//! The 32 header bytes are plaintext, unauthenticated, and stored where anyone
//! with write access to the object can edit them (spec: FM-2). One of them is a
//! length, and every reader does something sized by it: collects that many bytes
//! into a buffer, or asks Storage for that many next. The AEAD would catch the
//! edit — afterwards, having already spent whatever the number asked for. These
//! cases are about the *before*: a declared meta section past
//! [`Header::MAX_META_LEN`] is refused for the price of the four bytes it took to
//! read it.
//!
//! Nothing here allocates anything: the objects under test are 32 bytes long,
//! which is the whole point — a case that had to allocate to prove the ceiling
//! would be proving the opposite.

use coffret_model::{ContainerKey, ContainerKind, Mtime};

use super::decode;
use super::testing::{container_id, encode_with, key, source, SMALL_CHUNK};
use crate::chunk_size::ChunkSize;
use crate::container_reader::ContainerOutline;
use crate::encode::encode;
use crate::encode_request::EncodeRequest;
use crate::entry_paths::entry_path;
use crate::entry_source::EntrySource;
use crate::error::Error;
use crate::header::Header;

/// A header declaring `meta_len`, and nothing behind it.
///
/// The object is deliberately no longer than its header: whatever a reader does
/// with the declaration, it cannot be reading a meta section that is there.
fn header_declaring(meta_len: u32) -> [u8; Header::LEN] {
    let mut bytes = Header {
        container_id: container_id(),
        chunk_size: ChunkSize::DEFAULT,
        meta_len: 0,
    }
    .to_bytes();
    bytes[28..32].copy_from_slice(&meta_len.to_be_bytes());
    bytes
}

/// Whether a refusal is the one this file is about.
fn is_too_long(error: &Error, declared: u64) -> bool {
    matches!(
        error,
        Error::MetaSectionTooLong { declared: stated, limit }
            if *stated == declared && *limit == u64::from(Header::MAX_META_LEN)
    )
}

// FM-2: the largest length the field can hold is the one an adversary reaches
// for, because it is the largest allocation four edited bytes can command. It is
// answered by the header parse itself, so every reader downstream inherits the
// answer.
#[test]
fn a_header_declaring_the_largest_possible_meta_section_is_refused() {
    let front = header_declaring(u32::MAX);
    let result = Header::parse(&front);
    assert!(
        result
            .as_ref()
            .err()
            .is_some_and(|error| is_too_long(error, u64::from(u32::MAX))),
        "expected a meta section of {} bytes to be refused, got {result:?}",
        u32::MAX
    );
}

// The two readings that decide how many bytes are asked for next: a range reader
// asks the header how long the front is, and a whole-object reader opens the
// front it collected. Both refuse before either has anything to size.
#[test]
fn neither_reader_sizes_anything_by_a_declaration_past_the_ceiling() {
    let front = header_declaring(u32::MAX);

    let prefix = ContainerOutline::prefix_len(&front);
    assert!(
        prefix
            .as_ref()
            .err()
            .is_some_and(|error| is_too_long(error, u64::from(u32::MAX))),
        "expected no prefix length to be worked out at all, got {prefix:?}",
    );

    let opened = ContainerOutline::open(&front, &key());
    assert!(
        opened
            .as_ref()
            .err()
            .is_some_and(|error| is_too_long(error, u64::from(u32::MAX))),
        "expected the outline to refuse the declaration, got {opened:?}",
    );

    let decoded = decode(&front, &key());
    assert!(
        decoded
            .as_ref()
            .err()
            .is_some_and(|error| is_too_long(error, u64::from(u32::MAX))),
        "expected the whole-object decode to refuse the declaration, got {decoded:?}",
    );
}

// The ceiling is a boundary and not an approximation: the length at it is a
// length a Container may carry, and one byte more is not.
#[test]
fn the_ceiling_itself_is_a_length_a_container_may_declare() {
    let at = Header::parse(&header_declaring(Header::MAX_META_LEN));
    assert!(
        at.is_ok_and(|header| header.meta_len == Header::MAX_META_LEN),
        "the ceiling is a declaration a reader takes",
    );

    let past = Header::parse(&header_declaring(Header::MAX_META_LEN + 1));
    assert!(
        past.as_ref()
            .err()
            .is_some_and(|error| is_too_long(error, u64::from(Header::MAX_META_LEN) + 1)),
        "one byte past it is not, got {past:?}",
    );
}

// A Container this build writes is one it reads back, so the ceiling has to sit
// far above what an entry table really costs. A meta section of an ordinary
// Container is kilobytes against a ceiling of tens of megabytes.
#[test]
fn the_ceiling_admits_the_containers_this_build_writes() {
    let content = b"the bytes of one Entry".to_vec();
    let object = encode_with(SMALL_CHUNK, &[source("albums/2024/spring.jpg", &content)]);
    let header = Header::parse(object.bytes()).expect("an encoded Container has a valid header");
    assert!(
        u64::from(header.meta_len) * 1000 < u64::from(Header::MAX_META_LEN),
        "a meta section of {} bytes is not far enough inside the {} ceiling",
        header.meta_len,
        Header::MAX_META_LEN,
    );
}

// And a Container with an entry table of some size still is: the ceiling is
// about the absurd, so a hundred Entries with real Entry Paths must not come
// close to it.
#[test]
fn the_ceiling_admits_a_container_of_many_entries() {
    let paths: Vec<String> = (0..100)
        .map(|index| format!("books/vol-{:04}/page-{index:03}.png", index / 20))
        .collect();
    let content = b"an Entry".to_vec();
    let entries: Vec<EntrySource<'_>> = paths
        .iter()
        .map(|path| {
            EntrySource::new(
                entry_path(path.clone()),
                Mtime::from_unix_seconds(1_700_000_000),
                &content,
            )
        })
        .collect();
    let key = ContainerKey::from_bytes([0x33; ContainerKey::BYTE_LEN]);
    let object = encode(&EncodeRequest::new(
        container_id(),
        ContainerKind::Pack,
        &key,
        &entries,
    ))
    .expect("encoding a hundred Entries succeeds");

    let header = Header::parse(object.bytes()).expect("an encoded Container has a valid header");
    assert!(
        u64::from(header.meta_len) * 100 < u64::from(Header::MAX_META_LEN),
        "a hundred Entries take {} meta bytes, against a {} ceiling",
        header.meta_len,
        Header::MAX_META_LEN,
    );
}
