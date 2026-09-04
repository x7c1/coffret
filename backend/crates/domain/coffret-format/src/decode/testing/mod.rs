//! Helpers shared by the decoder's tests.

use coffret_model::{ContainerId, ContainerKey, ContainerKind, Mtime};

use crate::aead::TAG_LEN;
use crate::chunk_size::ChunkSize;
use crate::encode::encode;
use crate::encode_request::EncodeRequest;
use crate::encoded_container::EncodedContainer;
use crate::entry_paths::entry_path;
use crate::entry_source::EntrySource;
use crate::header::Header;

/// A chunk size small enough that a few dozen bytes span several chunks.
pub(super) const SMALL_CHUNK: u32 = 16;

pub(super) fn key() -> ContainerKey {
    ContainerKey::from_bytes([0x11; ContainerKey::BYTE_LEN])
}

pub(super) fn container_id() -> ContainerId {
    ContainerId::from_bytes([0x22; ContainerId::BYTE_LEN])
}

pub(super) fn source<'a>(path: &str, content: &'a [u8]) -> EntrySource<'a> {
    EntrySource::new(
        entry_path(path),
        Mtime::from_unix_seconds(1_700_000_000),
        content,
    )
}

pub(super) fn encode_with(chunk_size: u32, entries: &[EntrySource<'_>]) -> EncodedContainer {
    let request = EncodeRequest {
        container_id: container_id(),
        kind: ContainerKind::Pack,
        key: &key(),
        chunk_size: ChunkSize::new(chunk_size).expect("the chunk size is non-zero"),
        entries,
    };
    encode(&request).expect("encoding a non-empty entry list succeeds")
}

/// Byte ranges of the chunk messages, in stream order.
pub(super) fn chunk_ranges(object: &[u8], chunk_size: u32) -> Vec<std::ops::Range<usize>> {
    let header = Header::parse(object).expect("the object has a valid header");
    let message_len = chunk_size as usize + TAG_LEN;
    let mut position = Header::LEN + header.meta_len as usize;
    let mut ranges = Vec::new();
    while position < object.len() {
        let take = message_len.min(object.len() - position);
        ranges.push(position..position + take);
        position += take;
    }
    ranges
}
