use coffret_model::{ContainerKey, ContentHash};

use crate::aead::{Cipher, TAG_LEN};
use crate::container_reader::ContainerOutline;
use crate::decoded_container::DecodedContainer;
use crate::decoded_entry::DecodedEntry;
use crate::error::{Error, Result};
use crate::header::Header;
use crate::nonce;
use crate::stream::StreamWriter;

#[cfg(test)]
mod rejection_tests;
#[cfg(test)]
mod round_trip_tests;
#[cfg(test)]
mod tampering_tests;

#[cfg(test)]
mod testing;

/// Opens a Container.
///
/// The header is validated on its plaintext bytes first, so an object that is
/// not a Container v1 is rejected before the key is used at all. After that
/// every chunk is authenticated before any of its bytes land in an Entry
/// buffer, and each recovered Entry is checked against its recorded hash.
///
/// The front of the object is read by
/// [`ContainerOutline`](crate::ContainerOutline), which is the same reading a
/// range reader does. What stays here is the walk over the chunk sequence, and
/// it stays because it answers a different question: this reader has the whole
/// object in hand, so a sequence that is shorter or longer than the meta section
/// describes is a *damaged object*, and what says so is the final-chunk domain
/// the last message present is then read under (spec: FM-7). A run reader is fed
/// bytes that are still arriving, where a short delivery is the provider's and
/// is named as that.
pub fn decode(object: &[u8], key: &ContainerKey) -> Result<DecodedContainer> {
    let outline = ContainerOutline::open(object, key)?;
    let associated_data = &object[..Header::LEN];
    let cipher = Cipher::new(key.as_bytes());

    let expected_len = outline.plaintext_len();
    // A chunk size beyond this platform's addressable range is not something
    // this reader can honor, even though the header is well formed.
    let chunk_bytes =
        usize::try_from(outline.chunk_size().get()).map_err(|_| Error::InvalidChunkSize)?;
    let mut writer = StreamWriter::new(
        outline.entries().iter().map(|entry| entry.size).collect(),
        outline.pad_len(),
        expected_len,
    );

    let body_start = usize::try_from(outline.body_start()).map_err(|_| Error::Truncated)?;
    let mut chunks = object.get(body_start..).ok_or(Error::Truncated)?;
    if chunks.is_empty() {
        return Err(Error::MissingChunks);
    }
    let mut index = 0u64;
    while !chunks.is_empty() {
        // Every non-final chunk is exactly one chunk size plus a tag, so the
        // last message in the object is the only one that can be shorter — and
        // the final-chunk domain in its nonce is what a truncated or extended
        // chunk sequence trips over.
        let message_len = chunk_bytes + TAG_LEN;
        let is_final = chunks.len() <= message_len;
        let take = if is_final { chunks.len() } else { message_len };
        let plaintext = cipher.open(
            &nonce::chunk(index, is_final),
            associated_data,
            &chunks[..take],
        )?;
        writer.write(&plaintext)?;
        chunks = &chunks[take..];
        index += 1;
    }

    if writer.written() != expected_len {
        return Err(Error::PlaintextLengthMismatch {
            expected: expected_len,
            actual: writer.written(),
        });
    }

    let container_id = outline.container_id();
    let chunk_size = outline.chunk_size();
    let kind = outline.kind();
    let entries = outline
        .into_entries()
        .into_iter()
        .zip(writer.into_contents())
        .enumerate()
        .map(|(index, (metadata, content))| {
            let hash = ContentHash::from_bytes(*blake3::hash(&content).as_bytes());
            if hash != metadata.hash {
                return Err(Error::ContentHashMismatch { index });
            }
            Ok(DecodedEntry { metadata, content })
        })
        .collect::<Result<Vec<_>>>()?;

    Ok(DecodedContainer {
        container_id,
        chunk_size,
        kind,
        entries,
    })
}
