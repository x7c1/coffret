use coffret_format::unwrap_container_key;
use coffret_model::{ContainerKey, ContainerSummary, ContentHash, KeyEnvelope, ObjectRef};
use tokio::io::AsyncReadExt;
use tracing::debug;

use crate::byte_stream::ByteStream;
use crate::commit::ControlListing;
use crate::error::{Error, Result};
use crate::fetch::decoding::Decoding;
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::placement::Placement;
use crate::fetch::target::Target;
use crate::fetch::TRANSFER_BUFFER;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// Fetches one Container and writes every wanted Entry beside its destination.
///
/// The fetch unit is the whole Container however many of its Entries are wanted
/// (spec: PK-16), so this happens once per Container in a run and never once per
/// Entry. The object is decoded as it arrives: nothing here holds more than a
/// transfer buffer, and each wanted Entry's plaintext goes straight into a
/// temporary file beside where its file will be.
///
/// Three checks, in the order that keeps each one meaningful:
///
/// 1. **The bytes are the bytes the record named.** BLAKE3-256 of the ciphertext
///    against what the Journal record recorded (spec: FM-15, CP-11). It is the
///    only one of the three that does not involve a key, and it is a claim about
///    the whole object, so it can only be settled once the last byte has passed
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
/// publish (spec: EP-11).
///
/// The handle comes from the Index where this device has one and from the walk
/// the catch-up made otherwise. A device that replayed a record has never seen
/// the object, so its summary caches no handle and the name its ID gives it is
/// how it is reached (spec: FM-3).
pub(super) async fn fetch<'a>(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    keys: &LibraryKeys,
    listing: &ControlListing,
    summary: &ContainerSummary,
    envelope: &KeyEnvelope,
    wanted: &'a [Target],
) -> FetchResult<Vec<Placement<'a>>> {
    let container_id = summary.id;
    let object: &ObjectRef = summary
        .object_ref
        .as_ref()
        .or_else(|| listing.container(container_id))
        .ok_or(FetchError::ContainerUnreachable { container_id })?;
    let key = unwrap_container_key(keys.container_wrap(), &container_id, envelope)?;

    // The whole read is inside the retry rather than only the call that opens
    // it: a stream that dies halfway is a call to make again, and the attempt
    // that makes it opens a fresh one and writes fresh temporary files — the
    // same contract the upload's re-opened spool file meets.
    let placements = retry
        .run("get", || async {
            let stream = store.get(object, None).await?;
            decode_into_place(stream, summary, &key, wanted).await
        })
        .await??;

    debug!(
        container = %container_id,
        object = %container_id.object_name(),
        bytes = summary.ciphertext_len,
        entries = placements.len(),
        "fetched a Container and wrote its wanted Entries beside their destinations",
    );
    Ok(placements)
}

/// One attempt: drain the object through the chunk decoder and onto disk.
///
/// The two error channels are two different answers. The outer one is Storage's
/// — a transfer that failed or came up short, which the policy may attempt again
/// — and the inner one is a verdict about the Library, which no later attempt
/// would change. Either way the temporary files this attempt made are gone
/// before it returns.
async fn decode_into_place<'a>(
    stream: ByteStream,
    summary: &ContainerSummary,
    key: &ContainerKey,
    wanted: &'a [Target],
) -> Result<FetchResult<Vec<Placement<'a>>>> {
    let expected = stream.len();
    let mut reader = stream.into_reader();
    let mut buffer = vec![0u8; TRANSFER_BUFFER];

    let mut hasher = blake3::Hasher::new();
    let mut decoding = Decoding::new(summary.id, key, wanted);
    // The first refusal the decode made, kept until the object's own hash has
    // had its say (see the three checks above).
    let mut held: Option<FetchError> = None;
    let mut received = 0u64;

    loop {
        let read = match reader.read(&mut buffer).await {
            Ok(read) => read,
            Err(cause) => {
                decoding.discard().await;
                return Err(Error::from(cause));
            }
        };
        if read == 0 {
            break;
        }
        received += read as u64;
        hasher.update(&buffer[..read]);
        if held.is_none() {
            if let Err(error) = decoding.absorb(&buffer[..read]).await {
                held = Some(error);
            }
        }
    }

    if received != expected {
        decoding.discard().await;
        return Err(Error::LengthMismatch {
            expected,
            actual: received,
        });
    }

    let actual = ContentHash::from_bytes(*hasher.finalize().as_bytes());
    if actual != summary.ciphertext_hash {
        decoding.discard().await;
        return Ok(Err(FetchError::CiphertextMismatch {
            container_id: summary.id,
            expected: summary.ciphertext_hash,
            actual,
        }));
    }
    if let Some(error) = held {
        decoding.discard().await;
        return Ok(Err(error));
    }

    Ok(decoding.verify().await)
}
