use coffret_model::{ContentHash, EntryMetadata};

use crate::aead::Cipher;
use crate::encode_request::EncodeRequest;
use crate::encoded_container::EncodedContainer;
use crate::error::Result;
use crate::layout::Layout;
use crate::nonce;
use crate::stream::StreamReader;

/// Lays out a Container: header, encrypted meta section, encrypted chunks.
///
/// The plaintext stream is every Entry's content in the order given, padded up
/// to its Padmé bucket; that stream is cut into chunks of the requested size and
/// each chunk is encrypted separately, so the padding tail is never
/// materialized and only one chunk of plaintext is buffered at a time.
///
/// Every Entry's content is in memory before the call, which is what an object
/// the size of one photo affords and a Pack does not:
/// [`ContainerWriter`](crate::ContainerWriter) writes the same bytes from a
/// declared entry table and a stream of content, for callers whose Container is
/// larger than what they are willing to hold.
pub fn encode(request: &EncodeRequest<'_>) -> Result<EncodedContainer> {
    let entries: Vec<EntryMetadata> = request
        .entries
        .iter()
        .map(|source| EntryMetadata {
            path: source.path.clone(),
            // Assigned by the layout, which is what makes the table describe
            // the stream it is written next to.
            offset: 0,
            size: source.content.len() as u64,
            mtime: source.mtime,
            hash: ContentHash::from_bytes(*blake3::hash(source.content).as_bytes()),
            derived_from: source.derived_from.clone(),
            mime: source.mime.clone(),
        })
        .collect();

    let mut layout = Layout::plan(
        request.container_id,
        request.chunk_size,
        request.kind,
        entries,
    )?;

    let cipher = Cipher::new(request.key.as_bytes());
    let mut object = Vec::with_capacity(layout.object_len_hint());
    object.extend_from_slice(&layout.header_bytes);
    cipher.seal(
        &nonce::meta(),
        &layout.header_bytes,
        &mut layout.meta_plaintext,
        &mut object,
    )?;

    let chunk_size = u64::from(request.chunk_size.get());
    let mut reader = StreamReader::new(request.entries, layout.pad_len);
    // A stream shorter than one chunk needs no more buffer than it fills.
    let buffer_len = usize::try_from(chunk_size.min(layout.padded_len).max(1))
        .expect("the buffer is at most one chunk long");
    let mut buffer = vec![0u8; buffer_len];
    for index in 0..layout.chunk_count {
        let filled = reader.read(&mut buffer);
        let is_final = index + 1 == layout.chunk_count;
        cipher.seal(
            &nonce::chunk(index, is_final),
            &layout.header_bytes,
            &mut buffer[..filled],
            &mut object,
        )?;
    }

    Ok(EncodedContainer::new(
        object,
        request.container_id.object_name(),
    ))
}
