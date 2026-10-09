use coffret_format::unwrap_container_key;
use coffret_model::{ContainerKey, ContainerSummary, ContentHash, KeyEnvelope, ObjectRef};
use tokio::io::AsyncReadExt;
use tracing::debug;

use crate::answer_length::AnswerLength;
use crate::byte_stream::ByteStream;
use crate::destinations::Destinations;
use crate::error::{Error, Result};
use crate::fetch::decoding::Decoding;
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::placement::Placed;
use crate::fetch::reading::Reading;
use crate::fetch::scratch_guard::ScratchGuard;
use crate::fetch::target::Target;
use crate::fetch::TRANSFER_BUFFER;

/// Fetches one Container and writes every wanted Entry beside its destination.
///
/// A read of the whole object is every parcel of it at once, which PK-16 allows
/// and a folder fetch wants: it happens once per Container in a run however
/// many of its Entries are wanted, and never once per Entry. Nothing of it is
/// kept as parcels, because every Entry it was read for is placed out of it.
/// The object is decoded as it arrives: nothing here holds more than a transfer
/// buffer, and each wanted Entry's plaintext goes straight into a scratch beside
/// where its file will be.
///
/// Three checks, in the order that keeps each one meaningful:
///
/// 1. **The bytes are the bytes the record named.** BLAKE3-256 of the ciphertext
///    against what the Journal record recorded (spec: FM-15, CP-11). It is the
///    only one of the three that does not involve a key, and it is a claim about
///    the whole object, so it can only be decided once the last byte has passed
///    — which is why a decode that fails part way is *held* rather than raised:
///    a substituted or damaged object should be reported as that, not as a
///    Container that would not open. If the object hashes to what the record
///    says after all, the held refusal is what comes back.
/// 2. **They authenticate.** The Container Key comes out of the envelope the
///    committed Keyring maps this Container to, unwrapped against the
///    Container's own ID (spec: FM-14, KL-7), and every chunk is authenticated
///    before any of its bytes reach a file (spec: FM-5, FM-8).
/// 3. **They are the content this catalog names.** Each Entry's plaintext is
///    hashed as it passes and held against what the Index records for it, which
///    is the placement's own last step before it may be published.
///
/// Nothing becomes visible until all three have passed: what comes back is a
/// Container's worth of verified, still-invisible files for the caller to
/// publish (spec: EP-11), together with any mapped root that would not vouch for
/// itself — those Entries are placed nowhere and the mapping is reported once,
/// while the Container's other Entries are placed as usual (spec: EP-13).
///
/// The handle comes from the Index where this device has one and from the walk
/// the catch-up made otherwise. A device that replayed a record has never seen
/// the object, so its summary caches no handle and the name its ID gives it is
/// how it is reached (spec: FM-3).
pub(super) async fn fetch<'a>(
    reading: &Reading<'a>,
    summary: &ContainerSummary,
    envelope: &KeyEnvelope,
    wanted: &'a [Target],
) -> FetchResult<Placed<'a>> {
    let container_id = summary.id;
    let object: &ObjectRef = summary
        .object_ref
        .as_ref()
        .or_else(|| reading.listing.container(container_id))
        .ok_or(FetchError::ContainerUnreachable { container_id })?;
    let key = unwrap_container_key(reading.keys.container_wrap(), &container_id, envelope)?;

    // The whole read is inside the retry rather than only the call that opens
    // it: a stream that dies halfway is a call to make again, and the attempt
    // that makes it opens a fresh one and writes fresh scratches — the
    // same contract the upload's re-opened spool file meets.
    let placed = reading
        .retry
        .run("get", || async {
            let stream = reading.store.get(object, None).await?;
            decode_into_place(stream, summary, &key, reading.destinations, wanted).await
        })
        .await??;

    debug!(
        container = %container_id,
        object = %container_id.object_name(),
        bytes = summary.ciphertext_len.get(),
        entries = placed.placements.len(),
        refused_roots = placed.refused.len(),
        "fetched a Container and wrote its wanted Entries beside their destinations",
    );
    Ok(placed)
}

/// One attempt: drain the object through the chunk decoder and onto disk.
///
/// The two error channels are two different answers. The outer one is Storage's
/// — a transfer that failed or came up short, which the policy may attempt again
/// — and the inner one is a verdict about the Library, which no later attempt
/// would change. Either way the scratches this attempt made are gone
/// before it returns: they are held by a [`ScratchGuard`] that only the success
/// disarms.
///
/// The object is held to the length the catalog records for it as well as to
/// the length the stream declares (spec: FM-15), and a difference from either is
/// Storage's. That is what makes the chunk decoder's own length refusals — a
/// chunk run that ended short, or went on past its last chunk — the Library's
/// when they come back: by the time a held refusal is returned, the object has
/// arrived at exactly its recorded length and hashed to its recorded hash, so it
/// is the object as it was committed, and a run that does not fit it is its
/// header lying about its own lengths. A provider answering short is caught
/// before that, as a length, where the retry policy sees it.
async fn decode_into_place<'a>(
    stream: ByteStream,
    summary: &ContainerSummary,
    key: &ContainerKey,
    destinations: &'a dyn Destinations,
    wanted: &'a [Target],
) -> Result<FetchResult<Placed<'a>>> {
    let mut length = AnswerLength::new(stream.len(), summary.ciphertext_len.get());
    let mut reader = stream.into_reader();
    let mut buffer = vec![0u8; TRANSFER_BUFFER];

    let mut hasher = blake3::Hasher::new();
    let mut decoding = ScratchGuard::armed(Decoding::new(summary.id, key, destinations, wanted));
    // The first refusal the decode made, kept until the object's own hash has
    // had its say (see the three checks above).
    let mut held: Option<FetchError> = None;

    loop {
        let read = reader.read(&mut buffer).await.map_err(Error::from)?;
        if read == 0 {
            break;
        }
        length.count(read)?;
        hasher.update(&buffer[..read]);
        if held.is_none() {
            if let Err(error) = decoding.absorb(&buffer[..read]).await {
                held = Some(error);
            }
        }
    }
    length.finish()?;

    let actual = ContentHash::from_bytes(*hasher.finalize().as_bytes());
    if actual != summary.ciphertext_hash {
        return Ok(Err(FetchError::CiphertextMismatch {
            container_id: summary.id,
            expected: summary.ciphertext_hash,
            actual,
        }));
    }
    if let Some(error) = held {
        return Ok(Err(error));
    }

    // Verifying discards what it made on its own refusal, so from here the
    // scratches are its to keep or remove.
    Ok(decoding.disarm().verify().await)
}
