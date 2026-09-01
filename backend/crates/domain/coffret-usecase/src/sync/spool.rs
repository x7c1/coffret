use std::path::Path;

use coffret_format::{
    encode, generate_container_id, generate_container_key, wrap_container_key, EncodeRequest,
    EntrySource,
};
use coffret_model::{ContainerKind, ContentHash, EntryMetadata};
use tracing::debug;

use crate::device_state::{BatchId, DeviceTime, PendingUpload, SpoolState};
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::spool_file::SpoolFile;
use crate::spooled_container::SpooledContainer;
use crate::sync::candidate::Candidate;
use crate::sync::sync_error::SyncResult;

/// Encodes one local file into a Container and writes it to the spool.
///
/// The order is the one an interruption has to survive. The pending row is
/// recorded before the spool file is created — not before it is finished, before
/// it exists — so there is no window in which ciphertext sits on this device
/// unnamed: from the instant a file can be at `spool_path` a row names it, and
/// every interruption from here on leaves the positive local provenance that
/// makes cleaning up possible at all (spec: OC-2, OC-3). The row is flipped to
/// [`Spooled`](crate::device_state::SpoolState::Spooled) once the file
/// is flushed, so what it says about the file changes at the moment the file
/// does. Nothing but this flow may write into the spool directory.
///
/// The wrap of the Container Key comes after that flip deliberately: a wrap
/// failure then leaves a `Spooled` row over a complete spool rather than a
/// `Spooling` row over one.
///
/// The whole file is in memory for the length of the call, which one file at a
/// time affords. A Pack does not, which is why [`freeze`](crate::freeze) spools
/// through the streaming encoder instead (spec: PK-5, FM-5).
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
    // (spec: FM-4, FM-9, PK-15). The birth time is the one value the scan read
    // that nothing here can derive, so it travels with the Entry.
    let entries = [EntrySource {
        btime: candidate.source.btime,
        ..EntrySource::new(
            candidate.source.path.clone(),
            candidate.source.mtime,
            &content,
        )
    }];
    let container = encode(&EncodeRequest::new(
        container_id,
        ContainerKind::OneFile,
        &container_key,
        &entries,
    ))?;
    let content_hash = ContentHash::from_bytes(*blake3::hash(&content).as_bytes());

    let spool_path = spool_dir.join(format!("{container_id}.spool"));
    index
        .record_pending_upload(PendingUpload {
            container_id,
            spool_path: spool_path.clone(),
            batch: batch.clone(),
            created_at: now,
            state: SpoolState::Spooling,
            object_ref: None,
        })
        .await?;

    let mut spool = SpoolFile::create(&spool_path).await?;
    spool.write(container.bytes()).await?;
    let digests = spool.finish().await?;
    index.mark_spooled(container_id).await?;

    let envelope = wrap_container_key(keys.container_wrap(), &container_id, &container_key)?;

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
            btime: candidate.source.btime,
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
