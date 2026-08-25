use std::path::Path;

use coffret_format::{
    encode, generate_container_id, generate_container_key, wrap_container_key, EncodeRequest,
    EntrySource,
};
use coffret_model::{ContainerKind, ContentHash, EntryMetadata};
use tracing::debug;

use crate::device_state::{BatchId, DeviceTime, PendingUpload};
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::spool_file::SpoolFile;
use crate::spooled_container::SpooledContainer;
use crate::sync::candidate::Candidate;
use crate::sync::sync_error::SyncResult;

/// Encodes one local file into a Container and writes it to the spool.
///
/// The order is the one an interruption has to survive. The Container is
/// encoded and written first, and the pending row is recorded before anything
/// is uploaded, so a run that dies mid-upload leaves a row naming what it
/// created — the positive local provenance that makes cleaning it up possible
/// (spec: OC-2, OC-3). A run that dies before the row exists leaves a spool
/// file nothing names, which is why the spool directory belongs to the sync and
/// to nothing else.
///
/// The whole file is in memory for the length of the call, which one file at a
/// time affords. A Pack does not, which is why [`freeze`](crate::freeze) spools
/// through the streaming encoder instead (spec: PK-3, PK-5).
pub(super) async fn spool(
    index: &dyn Index,
    keys: &LibraryKeys,
    spool_dir: &Path,
    batch: &BatchId,
    now: DeviceTime,
    candidate: &Candidate,
) -> SyncResult<SpooledContainer> {
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
    let mut spool = SpoolFile::create(&spool_path).await?;
    spool.write(container.bytes()).await?;
    let digests = spool.finish().await?;
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
    Ok(SpooledContainer {
        container_id,
        kind: ContainerKind::OneFile,
        spool_path,
        entries: vec![EntryMetadata {
            path: candidate.source.path.clone(),
            offset: 0,
            size: content.len() as u64,
            mtime: candidate.source.mtime,
            hash: content_hash,
            derived_from: None,
            mime: None,
        }],
        envelope,
        ciphertext_hash: digests.blake3,
        ciphertext_len: digests.len,
        provider_digest: digests.md5,
        object_ref: None,
        replaces: candidate.replaces.into_iter().collect(),
    })
}
