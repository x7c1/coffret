//! Objects that are refused on their shape rather than on a failed tag.

use coffret_model::{ContainerKey, ContainerKind};

use super::decode;
use super::testing::{container_id, encode_with, key, source, SMALL_CHUNK};
use crate::aead::{Cipher, TAG_LEN};
use crate::chunk_size::ChunkSize;
use crate::encode::encode;
use crate::encode_request::EncodeRequest;
use crate::error::Error;
use crate::header::Header;
use crate::meta::Meta;
use crate::nonce;

// FM-2: an object whose magic is not "CFRT1" is rejected without attempting
// decryption — the wrong key here never gets used.
#[test]
fn unknown_magic_is_rejected_before_decryption() {
    let content = b"payload".to_vec();
    let mut object = encode_with(SMALL_CHUNK, &[source("a", &content)]).into_bytes();
    object[..5].copy_from_slice(b"CFCTL");
    let wrong_key = ContainerKey::from_bytes([0xff; ContainerKey::BYTE_LEN]);
    assert_eq!(
        decode(&object, &wrong_key),
        Err(Error::UnknownMagic { actual: *b"CFCTL" })
    );
}

// FM-2: an object whose format version is unknown is rejected without
// attempting decryption.
#[test]
fn unknown_format_version_is_rejected_before_decryption() {
    let content = b"payload".to_vec();
    let mut object = encode_with(SMALL_CHUNK, &[source("a", &content)]).into_bytes();
    object[5] = 0x02;
    let wrong_key = ContainerKey::from_bytes([0xff; ContainerKey::BYTE_LEN]);
    assert_eq!(
        decode(&object, &wrong_key),
        Err(Error::UnsupportedVersion { actual: 0x02 })
    );
}

// FM-10: a Container lists at least one Entry, so encoding an empty entry
// list is refused.
#[test]
fn encoding_an_empty_entry_list_is_rejected() {
    let container_key = key();
    let request = EncodeRequest::new(container_id(), ContainerKind::Pack, &container_key, &[]);
    assert_eq!(encode(&request), Err(Error::EmptyEntryTable));
}

// FM-10: a Container lists at least one Entry, so an otherwise
// well-formed object whose entry table is empty is refused on decode.
#[test]
fn decoding_an_empty_entry_table_is_rejected() {
    let meta = Meta {
        kind: ContainerKind::Pack,
        pad_len: 0,
        entries: Vec::new(),
    };
    let mut meta_plaintext = crate::meta::encode(&meta).expect("encoding succeeds");
    let header = Header {
        container_id: container_id(),
        chunk_size: ChunkSize::DEFAULT,
        meta_len: (meta_plaintext.len() + TAG_LEN) as u32,
    };
    let header_bytes = header.to_bytes();
    let cipher = Cipher::new(&key());

    let mut object = header_bytes.to_vec();
    cipher
        .seal(
            &nonce::meta(),
            &header_bytes,
            &mut meta_plaintext,
            &mut object,
        )
        .expect("sealing succeeds");
    cipher
        .seal(&nonce::chunk(0, true), &header_bytes, &mut [], &mut object)
        .expect("sealing succeeds");

    assert_eq!(decode(&object, &key()), Err(Error::EmptyEntryTable));
}

#[test]
fn an_object_with_no_chunks_is_rejected() {
    let content = b"payload".to_vec();
    let object = encode_with(SMALL_CHUNK, &[source("a", &content)]).into_bytes();
    let header = Header::parse(&object).expect("the object has a valid header");
    let without_chunks = object[..Header::LEN + header.meta_len as usize].to_vec();
    assert_eq!(decode(&without_chunks, &key()), Err(Error::MissingChunks));
}
