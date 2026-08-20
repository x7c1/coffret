//! Edits to an encoded object that authentication has to catch.

use coffret_model::ContainerKey;

use super::decode;
use super::testing::{chunk_ranges, encode_with, key, source, SMALL_CHUNK};
use crate::aead::TAG_LEN;
use crate::decoded_container::DecodedContainer;
use crate::error::{Error, Result};
use crate::header::Header;

/// Asserts that an edited object was refused by authentication.
///
/// Every test here makes one edit and expects the same verdict, so naming the
/// edit is what a failure needs to report. `#[track_caller]` keeps the reported
/// panic location on the test that made the edit rather than on this helper.
#[track_caller]
fn assert_authentication_failed(result: Result<DecodedContainer>, edit: &str) {
    assert!(
        matches!(result, Err(Error::AuthenticationFailed)),
        "{edit} should fail authentication, got {result:?}"
    );
}

// FM-8: the associated data of every AEAD message is the full 32-byte
// header, so altering the Container ID fails decryption.
#[test]
fn tampering_with_the_container_id_fails_decryption() {
    let content = vec![0x5au8; 50];
    let mut object = encode_with(SMALL_CHUNK, &[source("a", &content)]).into_bytes();
    object[8] ^= 0x01;
    assert_authentication_failed(decode(&object, &key()), "an altered Container ID");
}

// FM-8: altering the chunk size recorded in the header fails decryption.
#[test]
fn tampering_with_the_chunk_size_fails_decryption() {
    let content = vec![0x5au8; 50];
    let mut object = encode_with(SMALL_CHUNK, &[source("a", &content)]).into_bytes();
    object[24..28].copy_from_slice(&32u32.to_be_bytes());
    assert_authentication_failed(decode(&object, &key()), "an altered chunk size");
}

// FM-8: altering the meta section length recorded in the header fails
// decryption.
#[test]
fn tampering_with_the_meta_section_length_fails_decryption() {
    let content = vec![0x5au8; 50];
    let mut object = encode_with(SMALL_CHUNK, &[source("a", &content)]).into_bytes();
    let header = Header::parse(&object).expect("the object has a valid header");
    object[28..32].copy_from_slice(&(header.meta_len + 1).to_be_bytes());
    assert_authentication_failed(decode(&object, &key()), "an altered meta section length");
}

// FM-7: the nonce counter makes reordering the chunk sequence fail
// authentication.
#[test]
fn reordering_chunks_fails_authentication() {
    let content = vec![0x5au8; 60];
    let object = encode_with(SMALL_CHUNK, &[source("a", &content)]).into_bytes();
    let ranges = chunk_ranges(&object, SMALL_CHUNK);
    assert!(ranges.len() >= 3, "the sample needs several chunks");

    let mut reordered = object.clone();
    let first = object[ranges[0].clone()].to_vec();
    let second = object[ranges[1].clone()].to_vec();
    reordered[ranges[0].clone()].copy_from_slice(&second);
    reordered[ranges[1].clone()].copy_from_slice(&first);
    assert_ne!(reordered, object, "the two chunks differ");
    assert_authentication_failed(decode(&reordered, &key()), "two chunks swapped");
}

// FM-7: the final-chunk domain makes dropping the last chunk fail
// authentication rather than yielding a shorter Container.
#[test]
fn dropping_the_final_chunk_fails_authentication() {
    let content = vec![0x5au8; 60];
    let object = encode_with(SMALL_CHUNK, &[source("a", &content)]).into_bytes();
    let ranges = chunk_ranges(&object, SMALL_CHUNK);
    let truncated = object[..ranges.last().expect("there is a final chunk").start].to_vec();
    assert_authentication_failed(decode(&truncated, &key()), "the final chunk dropped");
}

// FM-7: truncating the object part-way through a chunk fails
// authentication.
#[test]
fn truncating_the_chunk_sequence_fails_authentication() {
    let content = vec![0x5au8; 60];
    let object = encode_with(SMALL_CHUNK, &[source("a", &content)]).into_bytes();
    let truncated = object[..object.len() - 4].to_vec();
    assert_authentication_failed(decode(&truncated, &key()), "a chunk cut short");
}

// FM-7: appending to the chunk sequence fails authentication, because the
// chunk that used to be final is now read under the non-final domain.
#[test]
fn extending_the_chunk_sequence_fails_authentication() {
    let content = vec![0x5au8; 60];
    let mut object = encode_with(SMALL_CHUNK, &[source("a", &content)]).into_bytes();
    object.extend_from_slice(&[0u8; SMALL_CHUNK as usize + TAG_LEN]);
    assert_authentication_failed(decode(&object, &key()), "a chunk appended");
}

#[test]
fn a_wrong_key_fails_authentication() {
    let content = b"payload".to_vec();
    let object = encode_with(SMALL_CHUNK, &[source("a", &content)]).into_bytes();
    let wrong_key = ContainerKey::from_bytes([0xff; ContainerKey::BYTE_LEN]);
    assert_authentication_failed(decode(&object, &wrong_key), "another Container key");
}
