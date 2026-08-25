use std::collections::BTreeMap;

use coffret_model::ContainerId;
use tokio::fs;
use tracing::{debug, info, warn};

use crate::byte_stream::ByteStream;
use crate::device_state::{BatchId, DeviceTime, PendingUpload};
use crate::error::Error;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::provider_hash::ProviderHash;
use crate::retry::RetryPolicy;
use crate::spooled_container::SpooledContainer;
use crate::upload::upload_error::UploadError;

/// How many listing pages the verification walk may take before the run stops
/// asking ([`UploadError::ListingLimitReached`]).
const MAX_PAGES: usize = 100_000;

/// Puts every spooled Container on Storage and confirms what arrived.
///
/// Each attempt opens the spool file again, which is the contract
/// [`RetryPolicy::run`] is shaped around: a [`ByteStream`] is consumed by the
/// attempt that failed, so what produces a fresh one is the caller that knows
/// where the bytes are.
///
/// The pending row is updated with the handle Storage answered with as soon as
/// each upload lands, before the next one starts. That is what makes an
/// interruption in the middle of a batch recoverable: the rows left behind say
/// which Containers reached Storage and which never left the device, which is
/// the difference between an object to dispose of and a file to delete
/// (spec: OC-2).
pub(crate) async fn upload(
    store: &dyn ObjectStore,
    index: &dyn Index,
    retry: &RetryPolicy,
    batch: &BatchId,
    now: DeviceTime,
    spooled: &mut [SpooledContainer],
) -> Result<(), UploadError> {
    for container in spooled.iter_mut() {
        let name = container.container_id.object_name();
        let len = container.ciphertext_len;
        let object = retry
            .run("put", || {
                let spool_path = container.spool_path.clone();
                let name = name.clone();
                async move {
                    let file = fs::File::open(&spool_path).await?;
                    store.put(&name, ByteStream::new(len, file)).await
                }
            })
            .await
            .map_err(|error| refused(container.container_id, error))?;

        index
            .record_pending_upload(PendingUpload {
                container_id: container.container_id,
                spool_path: container.spool_path.clone(),
                batch: batch.clone(),
                created_at: now,
                object_ref: Some(object.clone()),
            })
            .await?;
        container.object_ref = Some(object);
        info!(
            container = %container.container_id,
            object = %name,
            bytes = len,
            entries = container.entries.len(),
            "uploaded a Container",
        );
    }
    verify(store, retry, spooled).await
}

/// Compares what the provider says it stored against what was sent.
///
/// The digest is not part of what a write answers with — S3 carries it on the
/// listing as an ETag, Drive as a checksum on the file resource — so the run
/// asks the listing once for the whole batch rather than once per object.
///
/// A provider that reports no digest for an object leaves the upload
/// unverified, and that is recorded rather than treated as a failure: the port
/// admits providers that report none, and the end-to-end guarantee is the
/// BLAKE3 of the ciphertext that the Journal record carries and a reader checks
/// after fetching (spec: FM-15, CP-11). A digest that disagrees is a different
/// matter — the object is not the bytes that were sent, so the run stops before
/// the batch names it.
async fn verify(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
    spooled: &[SpooledContainer],
) -> Result<(), UploadError> {
    if spooled.is_empty() {
        return Ok(());
    }
    let reported = digests(store, retry).await?;

    for container in spooled {
        let name = container.container_id.object_name();
        let Some(hash) = reported.get(&name) else {
            warn!(
                container = %container.container_id,
                object = %name,
                "Storage reported no digest for an uploaded Container, so nothing confirms \
                 it arrived whole",
            );
            continue;
        };
        if !hash
            .as_str()
            .eq_ignore_ascii_case(&container.provider_digest)
        {
            return Err(UploadError::TransferCorrupted {
                container_id: container.container_id,
                expected: container.provider_digest.clone(),
                actual: hash.as_str().to_owned(),
            });
        }
        debug!(
            container = %container.container_id,
            object = %name,
            "Storage stores the bytes that were sent",
        );
    }
    Ok(())
}

/// The digest Storage reports for each object it holds one for.
async fn digests(
    store: &dyn ObjectStore,
    retry: &RetryPolicy,
) -> Result<BTreeMap<String, ProviderHash>, UploadError> {
    let mut reported = BTreeMap::new();
    let mut token = None;
    for _ in 0..MAX_PAGES {
        let page = retry.run("list", || store.list(token.as_ref())).await?;
        for object in page.objects {
            if let Some(hash) = object.hash {
                reported.insert(object.name, hash);
            }
        }
        token = page.next;
        if token.is_none() {
            return Ok(reported);
        }
    }
    Err(UploadError::ListingLimitReached { pages: MAX_PAGES })
}

/// What a failed upload of one Container means, in the vocabulary its caller
/// matches on.
///
/// A provider that verifies the write on its own side reports a digest
/// disagreement in the port's vocabulary, and [`verify`] reaches the same
/// verdict from the listing a moment later. Keeping one spelling means a caller
/// matches one variant rather than two, and the Container it is about is what
/// this layer knows and the port does not. Everything else Storage answers with
/// travels unchanged.
fn refused(container_id: ContainerId, error: Error) -> UploadError {
    match error {
        Error::IntegrityMismatch { expected, actual } => UploadError::TransferCorrupted {
            container_id,
            expected,
            actual,
        },
        error => UploadError::Storage(error),
    }
}
