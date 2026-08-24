use std::path::Path;

use coffret_format::{
    encode, generate_container_id, generate_container_key, wrap_container_key, EncodeRequest,
    EntrySource,
};
use coffret_model::{ContainerKind, ContentHash, EntryMetadata};
use md5::{Digest, Md5};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::debug;

use crate::device_state::{BatchId, DeviceTime, PendingUpload};
use crate::index::Index;
use crate::sync::candidate::Candidate;
use crate::sync::spooled::Spooled;
use crate::sync::sync_error::{LocalOperation, SyncError, SyncResult};
use crate::sync::sync_keys::SyncKeys;

/// How much of the ciphertext is hashed and written at a time.
///
/// The two digests are folded in as the bytes go to disk rather than by reading
/// the spool back, which is what makes the size of one write a question at all.
/// 64 KiB is large enough that the syscall cost disappears against the bytes it
/// moves.
const WRITE_CHUNK: usize = 64 * 1024;

/// Encodes one local file into a Container and writes it to the spool.
///
/// The order is the one an interruption has to survive. The Container is
/// encoded and written first, and the pending row is recorded before anything
/// is uploaded, so a run that dies mid-upload leaves a row naming what it
/// created — the positive local provenance that makes cleaning it up possible
/// (spec: OC-2, OC-3). A run that dies before the row exists leaves a spool
/// file nothing names, which is why the spool directory belongs to the sync and
/// to nothing else.
pub(super) async fn spool(
    index: &dyn Index,
    keys: &SyncKeys,
    spool_dir: &Path,
    batch: &BatchId,
    now: DeviceTime,
    candidate: &Candidate,
) -> SyncResult<Spooled> {
    let content = candidate.source.read().await?;
    let container_id = generate_container_id()?;
    let container_key = generate_container_key()?;

    // One Entry, laid at offset zero, and no MIME: detection is not a sync's
    // work, and the encoder derives the offset, the size, and the hash from the
    // bytes themselves so none of the three can disagree with what is stored
    // (spec: FM-4, FM-9, PK-15).
    let entries = [EntrySource::new(
        candidate.source.path.clone(),
        candidate.source.mtime,
        &content,
    )];
    let container = encode(&EncodeRequest::new(
        container_id,
        ContainerKind::OneFile,
        &container_key,
        &entries,
    ))?;
    let content_hash = ContentHash::from_bytes(*blake3::hash(&content).as_bytes());

    let spool_path = spool_dir.join(format!("{container_id}.spool"));
    let digests = write(&spool_path, container.bytes()).await?;
    let envelope = wrap_container_key(keys.container_wrap(), &container_id, &container_key)?;

    index
        .record_pending_upload(PendingUpload {
            container_id,
            spool_path: spool_path.clone(),
            batch: batch.clone(),
            created_at: now,
            object_ref: None,
        })
        .await?;

    debug!(
        container = %container_id,
        bytes = digests.len,
        replaces = ?candidate.replaces.map(|id| id.to_string()),
        "encoded a Container and spooled it",
    );
    Ok(Spooled {
        container_id,
        spool_path,
        entry: EntryMetadata {
            path: candidate.source.path.clone(),
            offset: 0,
            size: content.len() as u64,
            mtime: candidate.source.mtime,
            hash: content_hash,
            derived_from: None,
            mime: None,
        },
        envelope,
        ciphertext_hash: digests.blake3,
        ciphertext_len: digests.len,
        provider_digest: digests.md5,
        object_ref: None,
        replaces: candidate.replaces,
    })
}

/// What writing a spool file produced besides the file.
struct Digests {
    blake3: ContentHash,
    md5: String,
    len: u64,
}

/// Writes the ciphertext out, hashing it on the way past.
///
/// Both digests are folded in as the bytes are written. Reading the file back
/// to hash it would double the I/O of every upload and would answer a different
/// question anyway — what is on disk now, rather than what was written.
async fn write(spool_path: &Path, bytes: &[u8]) -> SyncResult<Digests> {
    let mut file = fs::File::create(spool_path)
        .await
        .map_err(|cause| SyncError::Io {
            operation: LocalOperation::Creating,
            path: spool_path.to_path_buf(),
            cause,
        })?;

    let mut blake3 = blake3::Hasher::new();
    let mut md5 = Md5::new();
    for chunk in bytes.chunks(WRITE_CHUNK) {
        blake3.update(chunk);
        md5.update(chunk);
        file.write_all(chunk).await.map_err(|cause| SyncError::Io {
            operation: LocalOperation::Writing,
            path: spool_path.to_path_buf(),
            cause,
        })?;
    }
    // Flushed to the device, because the point of a spool is to be there after
    // the run that wrote it is not.
    file.sync_all().await.map_err(|cause| SyncError::Io {
        operation: LocalOperation::Flushing,
        path: spool_path.to_path_buf(),
        cause,
    })?;

    Ok(Digests {
        blake3: ContentHash::from_bytes(*blake3.finalize().as_bytes()),
        md5: md5
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
        len: bytes.len() as u64,
    })
}

/// Removes one spool file, its Container having been committed or abandoned.
///
/// A file that is already gone is the same outcome as one this call removed, so
/// an interrupted cleanup is simply run again (spec: OC-6).
pub(super) async fn discard(spool_path: &Path) -> SyncResult<()> {
    match fs::remove_file(spool_path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(cause) => Err(SyncError::Io {
            operation: LocalOperation::Removing,
            path: spool_path.to_path_buf(),
            cause,
        }),
    }
}
