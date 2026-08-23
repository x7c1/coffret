use coffret_model::{ContentHash, EntryMetadata};

use crate::aead::{Cipher, TAG_LEN};
use crate::encode_request::EncodeRequest;
use crate::encoded_container::EncodedContainer;
use crate::error::{Error, Result};
use crate::header::Header;
use crate::meta::{self, Meta};
use crate::nonce;
use crate::padme;
use crate::stream::StreamReader;

/// Lays out a Container: header, encrypted meta section, encrypted chunks.
///
/// The plaintext stream is every Entry's content in the order given, padded up
/// to its Padmé bucket; that stream is cut into chunks of the requested size and
/// each chunk is encrypted separately, so the padding tail is never
/// materialized and only one chunk of plaintext is buffered at a time.
pub fn encode(request: &EncodeRequest<'_>) -> Result<EncodedContainer> {
    // A Container exists only to hold user data, so an empty one is not a
    // Container worth writing.
    if request.entries.is_empty() {
        return Err(Error::EmptyEntryTable);
    }

    let mut entries = Vec::with_capacity(request.entries.len());
    let mut offset = 0u64;
    for source in request.entries {
        let size = source.content.len() as u64;
        entries.push(EntryMetadata {
            path: source.path.clone(),
            offset,
            size,
            mtime: source.mtime,
            hash: ContentHash::from_bytes(*blake3::hash(source.content).as_bytes()),
            derived_from: source.derived_from.clone(),
            mime: source.mime.clone(),
        });
        offset = offset.checked_add(size).ok_or(Error::StreamTooLong)?;
    }

    let unpadded_len = offset;
    let padded_len = padme::padded_len(unpadded_len);
    let meta = Meta {
        kind: request.kind,
        pad_len: padded_len - unpadded_len,
        entries,
    };

    // The header's associated data covers the meta section length, so the meta
    // section has to be serialized before the header can be written.
    //
    // Its plaintext is the CBOR map followed by zero padding up to the next
    // Padmé bucket, so the length the header records blurs the Entry count and
    // the total Entry Path length the way the stream padding blurs the content.
    // CBOR is self-delimiting, so the decoder needs no length field to tell the
    // map from the padding.
    let mut meta_plaintext = meta::encode(&meta)?;
    // The header records the padded section with its tag in one 32-bit field,
    // and the section is materialized in memory, so a meta section fits under
    // whichever of those two ceilings is lower.
    let limit = (u64::from(u32::MAX) - TAG_LEN as u64).min(usize::MAX as u64);
    let padded = padme::padded_len(meta_plaintext.len() as u64);
    if padded > limit {
        return Err(Error::MetaSectionTooLong { padded, limit });
    }
    let padded_meta_len = usize::try_from(padded).expect("checked against the ceiling above");
    meta_plaintext.resize(padded_meta_len, 0);
    let meta_len = u32::try_from(padded_meta_len + TAG_LEN).expect("checked against the ceiling");

    let header = Header {
        container_id: request.container_id,
        chunk_size: request.chunk_size,
        meta_len,
    };
    let header_bytes = header.to_bytes();

    let cipher = Cipher::new(request.key.as_bytes());
    let chunk_size = u64::from(request.chunk_size.get());
    // Entries that are all empty still produce one empty final chunk, so every
    // object ends with a final-chunk message marking the end of the stream.
    let chunk_count = padded_len.div_ceil(chunk_size).max(1);

    let chunk_bytes = padded_len.saturating_add(chunk_count.saturating_mul(TAG_LEN as u64));
    let mut object = Vec::with_capacity(
        Header::LEN + meta_len as usize + usize::try_from(chunk_bytes).unwrap_or(0),
    );
    object.extend_from_slice(&header_bytes);
    cipher.seal(
        &nonce::meta(),
        &header_bytes,
        &mut meta_plaintext,
        &mut object,
    )?;

    let mut reader = StreamReader::new(request.entries, meta.pad_len);
    // A stream shorter than one chunk needs no more buffer than it fills.
    let buffer_len = usize::try_from(chunk_size.min(padded_len).max(1))
        .expect("the buffer is at most one chunk long");
    let mut buffer = vec![0u8; buffer_len];
    for index in 0..chunk_count {
        let filled = reader.read(&mut buffer);
        let is_final = index + 1 == chunk_count;
        cipher.seal(
            &nonce::chunk(index, is_final),
            &header_bytes,
            &mut buffer[..filled],
            &mut object,
        )?;
    }

    Ok(EncodedContainer::new(
        object,
        request.container_id.object_name(),
    ))
}
