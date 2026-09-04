//! Helpers shared by the meta section's tests.

use ciborium::Value;
use coffret_model::{ContainerKind, ContentHash, EntryMetadata, Mtime};

use super::{encode, Meta};
use crate::entry_paths::entry_path;
use crate::padme;

/// An entry that tiles the stream from `offset` for `size` bytes.
pub(super) fn entry(path: &str, offset: u64, size: u64) -> EntryMetadata {
    EntryMetadata {
        path: entry_path(path),
        offset,
        size,
        mtime: Mtime::from_unix_seconds(1_700_000_000),
        btime: None,
        hash: ContentHash::from_bytes([1u8; ContentHash::BYTE_LEN]),
        derived_from: None,
        mime: None,
    }
}

/// A two-entry Pack whose stream carries a padding tail.
pub(super) fn sample() -> Meta {
    Meta {
        kind: ContainerKind::Pack,
        pad_len: 7,
        entries: vec![entry("a.txt", 0, 4), entry("b.txt", 4, 9)],
    }
}

/// The meta section as CBOR, so a test can assert on the wire shape itself.
pub(super) fn as_value(meta: &Meta) -> Value {
    ciborium::from_reader(encode(meta).expect("encoding succeeds").as_slice())
        .expect("the meta section is valid CBOR")
}

/// The sample's plaintext, and where its CBOR map ends inside it.
///
/// The padding tail is what the cases around the map's end work on, so this
/// asserts there is one rather than leaving a case that found none quietly
/// passing.
pub(super) fn sample_plaintext() -> (Vec<u8>, usize) {
    let map = encode(&sample()).expect("encoding succeeds");
    let map_len = map.len();
    let plaintext = padded(map);
    assert!(
        map_len < plaintext.len(),
        "this meta section carries no padding to check"
    );
    (plaintext, map_len)
}

/// A hand-built CBOR value back as the bytes a writer would have produced.
pub(super) fn to_bytes(value: &Value) -> Vec<u8> {
    let mut bytes = Vec::new();
    ciborium::into_writer(value, &mut bytes).expect("writing a Value succeeds");
    padded(bytes)
}

/// A CBOR map carried to its Padmé bucket, which is the plaintext a meta
/// section is stored as (FM-9).
pub(super) fn padded(mut map: Vec<u8>) -> Vec<u8> {
    let padded_len = usize::try_from(padme::padded_len(map.len() as u64))
        .expect("a map this size fits in memory");
    map.resize(padded_len, 0);
    map
}
