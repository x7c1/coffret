use coffret_format::{decode, unwrap_container_key, DecodedContainer};
use coffret_model::{ContainerSummary, ContentHash, KeyEnvelope};
use tracing::debug;

use crate::commit::ControlListing;
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// Fetches one Container and opens it.
///
/// The fetch unit is the whole Container however many of its Entries are wanted
/// (spec: PK-16), so this happens once per Container in a run and never once per
/// Entry. Range-reading a single Entry out of a Pack is the viewer's optimization
/// and not this flow's; here the object is pulled whole and held in memory,
/// which is what image-sized Entries make affordable.
///
/// Three checks, in the order that keeps each one meaningful:
///
/// 1. **The bytes are the bytes the record named.** BLAKE3-256 of the ciphertext
///    against what the Journal record recorded (spec: FM-15, CP-11). It comes
///    first because it is the only one that does not involve a key: a substituted
///    or damaged object is refused without ever being presented to one.
/// 2. **They authenticate.** The Container Key comes out of the envelope the
///    committed Keyring maps this Container to, unwrapped against the
///    Container's own ID (spec: FM-14, KL-7), and the decode authenticates every
///    chunk before any of its bytes reach a buffer (spec: FM-5, FM-8).
/// 3. **They are the content this catalog names.** That check is the caller's,
///    per Entry, since the caller is what holds the catalog — but it is only
///    worth asking because the first two passed.
///
/// The handle comes from the Index where this device has one and from the walk
/// the catch-up made otherwise. A device that replayed a record has never seen
/// the object, so its summary caches no handle and the name its ID gives it is
/// how it is reached (spec: FM-3).
pub(super) async fn open(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    keys: &LibraryKeys,
    listing: &ControlListing,
    summary: &ContainerSummary,
    envelope: &KeyEnvelope,
) -> FetchResult<DecodedContainer> {
    let container_id = summary.id;
    let object = summary
        .object_ref
        .as_ref()
        .or_else(|| listing.container(container_id))
        .ok_or(FetchError::ContainerUnreachable { container_id })?;

    // Drained inside the retry rather than after it: a stream that dies halfway
    // is a call to make again, and the attempt that makes it opens a fresh one —
    // the same contract the upload's re-opened spool file meets.
    let ciphertext = retry
        .run("get", || async move {
            store.get(object, None).await?.into_bytes().await
        })
        .await?;

    let actual = ContentHash::from_bytes(*blake3::hash(&ciphertext).as_bytes());
    if actual != summary.ciphertext_hash {
        return Err(FetchError::CiphertextMismatch {
            container_id,
            expected: summary.ciphertext_hash,
            actual,
        });
    }

    let key = unwrap_container_key(keys.container_wrap(), &container_id, envelope)?;
    let container = decode(&ciphertext, &key)?;

    debug!(
        container = %container_id,
        object = %container_id.object_name(),
        bytes = ciphertext.len(),
        entries = container.entries.len(),
        "fetched a Container and opened it",
    );
    Ok(container)
}
