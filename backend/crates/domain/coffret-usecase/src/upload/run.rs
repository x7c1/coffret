use std::sync::Arc;

use coffret_model::ContainerId;
use tracing::{debug, info, warn};

use crate::byte_stream::ByteStream;
use crate::device_state::{BatchId, PendingRow, SpoolState};
use crate::error::Error;
use crate::index::Index;
use crate::local_io_error::LocalIoError;
use crate::object_store::ObjectStore;
use crate::progress::{ByteCount, Phase, Progress, Step};
use crate::provider_hash::ProviderHash;
use crate::retry::RetryPolicy;
use crate::spool::Spool;
use crate::spooled_container::SpooledContainer;
use crate::upload::pulled::{self, Pulled};
use crate::upload::upload_error::UploadError;

/// Puts every spooled Container on Storage and confirms what arrived.
///
/// Each attempt opens the spool file again, which is the contract
/// [`RetryPolicy::run`] is shaped around: a [`ByteStream`] is consumed by the
/// attempt that failed, so what produces a fresh one is the caller that knows
/// where the bytes are.
///
/// The progress is reported per Container and in bytes (see
/// [`Step`]'s rule for them). The port takes a whole body per call and says
/// nothing until it answers, so the bytes are counted where this layer can see
/// them go: off the stream each put is handed, as the store pulls it, and
/// reported every [`REPORT_EVERY`](super::pulled::REPORT_EVERY) while the put
/// is under way.
///
/// The pending row is updated with the handle Storage answered with as soon as
/// each upload lands, before the next one starts and before its digest is
/// compared. That is what makes an interruption in the middle of a batch
/// recoverable: the rows left behind say which Containers reached Storage and
/// which never left the device, which is the difference between an object to
/// dispose of and a file to delete (spec: OC-2) — and an object that did not
/// arrive whole is one to dispose of like any other.
pub(crate) async fn upload(
    store: &dyn ObjectStore,
    index: &dyn Index,
    spool: &dyn Spool,
    retry: &RetryPolicy,
    batch: &BatchId,
    progress: &dyn Progress,
    spooled: &mut [SpooledContainer],
) -> Result<(), UploadError> {
    // Said before the first object leaves, because the first one is where a
    // run that cannot reach Storage at all spends its retries.
    let total = spooled.len();
    let bytes_total = spooled
        .iter()
        .map(|container| container.ciphertext_len.get())
        .sum();
    let step = |done: usize, bytes_done: u64| {
        Step::new(Phase::Uploading, done, total).with_bytes(ByteCount {
            done: bytes_done,
            total: bytes_total,
        })
    };
    progress.step(step(0, 0));

    // What the Containers before the one in flight came to.
    let mut sent = 0;
    for (done, container) in spooled.iter_mut().enumerate() {
        let name = container.container_id.object_name();
        let len = container.ciphertext_len.get();
        let pulled = Pulled::default();
        let mut reported = sent;
        let put = retry.run("put", || {
            let spool_path = container.spool_path.clone();
            let name = name.clone();
            let pulled = pulled.clone();
            async move {
                let reader = spool.reader(&spool_path).await.map_err(unreadable)?;
                store
                    .put(&name, ByteStream::new(len, pulled.counting(reader)))
                    .await
            }
        });
        let uploaded = pulled::reporting(put, || {
            // Held to the object's length: the store reads one byte past it
            // to tell an exact body from a long one, and that byte is not one
            // more of the phase.
            let now = sent + pulled.get().min(len);
            if now != reported {
                reported = now;
                progress.step(step(done, now));
            }
        })
        .await
        .map_err(|error| refused(container.container_id, error))?;
        sent += len;

        index
            .record_pending_row(PendingRow {
                container_id: container.container_id,
                spool_path: container.spool_path.clone(),
                batch: batch.clone(),
                // The moment the spool was announced, which recording the
                // upload does not move.
                created_at: container.announced_at,
                // A Container reaches Storage only out of a finished spool, so
                // the row this replaces already said as much.
                state: SpoolState::Spooled(Some(uploaded.object_ref.clone())),
            })
            .await?;
        container.object_ref = Some(uploaded.object_ref);
        verify(container, uploaded.hash.as_ref())?;
        info!(
            container = %container.container_id,
            object = %name,
            bytes = len,
            entries = container.entries.len(),
            "uploaded a Container",
        );
        progress.step(step(done + 1, sent));
    }
    Ok(())
}

/// Compares what the provider says it stored against what was sent.
///
/// The digest is the one Storage answered the write with, so the object it
/// speaks for is the one this upload created — not whichever object a listing
/// happens to file under the same name.
///
/// A provider that reports no digest for an object leaves the upload
/// unverified, and that is recorded rather than treated as a failure: the port
/// admits providers that report none, and the end-to-end guarantee is the
/// BLAKE3 of the ciphertext that the Journal record carries and a reader checks
/// after fetching (spec: FM-15, CP-11). A digest that disagrees is a different
/// matter — the object is not the bytes that were sent, so the run stops before
/// the batch names it.
fn verify(
    container: &SpooledContainer,
    reported: Option<&ProviderHash>,
) -> Result<(), UploadError> {
    let Some(hash) = reported else {
        warn!(
            container = %container.container_id,
            "Storage reported no digest for an uploaded Container, so nothing confirms \
             it arrived whole",
        );
        return Ok(());
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
        "Storage stores the bytes that were sent",
    );
    Ok(())
}

/// A spool this device cannot read, in the vocabulary the transfer speaks.
///
/// The local end of a transfer is what [`Error::Io`] is for, and it is what a
/// spool that will not open is: the call never reached Storage, so nothing here
/// says anything about the provider. The cause travels as the value the
/// operating system produced — its [`kind`](std::io::ErrorKind) is what
/// separates a full disk from a file that is gone — and it is the whole of what
/// crosses: [`Error`] is the Storage vocabulary, which has no place for a local
/// operation or a local path. Nothing is lost by that here, because this
/// converts one call and one only — opening a finished spool — so the operation
/// it drops is a constant, and the path it drops is the one the caller passed
/// in.
fn unreadable(error: LocalIoError) -> Error {
    Error::Io {
        cause: Arc::new(error.cause),
    }
}

/// What a failed upload of one Container means, in the vocabulary its caller
/// matches on.
///
/// A provider that verifies the write on its own side reports a digest
/// disagreement in the port's vocabulary, and [`verify`] reaches the same
/// verdict from the digest a write answers with. Keeping one spelling means a caller
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
