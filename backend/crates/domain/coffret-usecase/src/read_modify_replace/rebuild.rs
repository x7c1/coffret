use std::path::Path;

use coffret_format::{
    generate_container_id, generate_container_key, unwrap_container_key, wrap_container_key,
    ChunkSize, ContainerWriter, EncodePlan, EntryPlan,
};
use coffret_model::{
    CiphertextLenClaim, ContainerId, ContainerKey, ContentHash, EntryMetadata, Redacted,
};
use tokio::io::AsyncReadExt;
use tracing::{debug, warn};

use crate::answer_length::AnswerLength;
use crate::byte_stream::ByteStream;
use crate::device_state::{PendingRow, SpoolState};
use crate::error::Error;
use crate::read_modify_replace::carrying::{Carrying, Stop};
use crate::read_modify_replace::rebuild_error::RebuildError;
use crate::read_modify_replace::rebuilding::Rebuilding;
use crate::read_modify_replace::replacing::Replacing;
use crate::read_modify_replace::unverified::Unverified;
use crate::read_modify_replace::TRANSFER_BUFFER;
use crate::spool_file::{Digests, SpoolFile};
use crate::spooled_container::SpooledContainer;

/// Reads one Container, verifies every Entry of it, and spools a replacement
/// carrying the kept ones forward (spec: PK-10).
///
/// The pending row naming the replacement is recorded before its spool file
/// exists, as every spool step's is (spec: OC-2), and it says the Container was
/// rebuilt rather than built out of local files: completing its bookkeeping
/// after an interrupted commit marks nothing present (spec: OC-7, EP-10).
///
/// The whole read is inside the retry, and each attempt writes the spool from
/// its first byte, the way a fetch's attempt writes fresh scratches: a stream
/// that died halfway leaves a spool that was being written, not one to resume.
///
/// Two answers besides success. [`Unverified`] is the old Container's verdict:
/// the spool and its row are disposed of on the spot — nothing was uploaded and
/// no commit was attempted, so both are this device's own leftovers
/// (spec: OC-2, OC-8) — and the caller commits nothing for that Container.
/// [`RebuildError`] is this device's or Storage's, and leaves the row for the
/// next sync to settle, as an interrupted freeze does.
pub(crate) async fn rebuild(
    with: &Rebuilding<'_>,
    replacing: &Replacing<'_>,
) -> Result<Result<SpooledContainer, Unverified>, RebuildError> {
    let old = replacing.old;
    debug_assert_eq!(replacing.table.len(), replacing.keep.len());
    debug_assert!(replacing.keep.iter().any(|keep| *keep));

    let Some(object) = old
        .object_ref
        .as_ref()
        .or_else(|| with.listing.container(old.id))
        .cloned()
    else {
        return Ok(Err(Unverified::Unreachable));
    };
    let old_key =
        match unwrap_container_key(with.keys.container_wrap(), &old.id, replacing.envelope) {
            Ok(key) => key,
            Err(error) => return Ok(Err(Unverified::Unopenable(error))),
        };

    let plans: Vec<EntryPlan> = replacing
        .table
        .iter()
        .zip(replacing.keep)
        .filter(|(_, keep)| **keep)
        .map(|(row, _)| EntryPlan::from(row))
        .collect();
    let container_id = generate_container_id()?;
    let container_key = generate_container_key()?;
    let spool_path = with.spool_dir.join(format!("{container_id}.spool"));
    with.index
        .record_pending_row(PendingRow {
            commit_attempted: false,
            // What it carries came off Storage, not off this device's disk.
            materializes: false,
            container_id,
            spool_path: spool_path.clone(),
            batch: with.batch.clone(),
            created_at: with.now,
            state: SpoolState::Spooling,
        })
        .await?;

    let plan = EncodePlan {
        container_id,
        kind: old.kind,
        key: &container_key,
        chunk_size: ChunkSize::DEFAULT,
        entries: &plans,
    };
    let attempted = with
        .retry
        .run("get", || async {
            let stream = with.store.get(&object, None).await?;
            attempt(with, replacing, &old_key, &plan, &spool_path, stream).await
        })
        .await;
    let stopped = match attempted {
        Ok(Ok((entries, digests))) => {
            with.index.mark_spooled(container_id).await?;
            let envelope =
                wrap_container_key(with.keys.container_wrap(), &container_id, &container_key)?;
            debug!(
                replaced = %old.id,
                container = %container_id,
                kept = entries.len(),
                omitted = replacing.table.len() - entries.len(),
                bytes = digests.len,
                "rebuilt a Container and spooled its replacement",
            );
            return Ok(Ok(SpooledContainer {
                container_id,
                kind: old.kind,
                spool_path,
                entries,
                envelope,
                ciphertext_hash: digests.blake3,
                ciphertext_len: CiphertextLenClaim::new(digests.len)
                    .map_err(coffret_format::Error::from)?,
                provider_digest: digests.md5,
                object_ref: None,
                announced_at: with.now,
                replaces: vec![old.id],
                materializes: false,
            }));
        }
        Ok(Err(Stop::Unverified(unverified))) => unverified,
        Ok(Err(Stop::Fault(error))) => return Err(error),
        // An object Storage says is not there is one there is nothing to read
        // out of, which is the old Container's verdict rather than the run's
        // (spec: FM-3).
        Err(Error::NotFound { .. }) => Unverified::Unreachable,
        Err(error) => return Err(error.into()),
    };

    warn!(
        container = %old.id,
        reason = %stopped.redacted(),
        "a Container could not be read and verified, so nothing replaces it",
    );
    dispose(with, container_id, &spool_path).await?;
    Ok(Err(stopped))
}

/// One attempt: drain the old object through the decoder and the kept part of
/// it into the replacement's spool.
///
/// The two channels are the fetch's: the outer one is Storage's, which the
/// retry policy may attempt again, and the inner one is everything else. A
/// verdict met part way is *held* until the object's own hash has had its say,
/// so that a substituted or damaged object is reported as that rather than as a
/// Container that would not open (spec: FM-15). A fault of this device's own
/// stops the attempt at once.
async fn attempt(
    with: &Rebuilding<'_>,
    replacing: &Replacing<'_>,
    old_key: &ContainerKey,
    plan: &EncodePlan<'_>,
    spool_path: &Path,
    stream: ByteStream,
) -> Result<Result<(Vec<EntryMetadata>, Digests), Stop>, Error> {
    let old = replacing.old;
    let mut length = AnswerLength::new(stream.len(), old.ciphertext_len.get());
    let mut reader = stream.into_reader();

    let mut file = match SpoolFile::create(with.spool, spool_path).await {
        Ok(file) => file,
        Err(error) => return Ok(Err(Stop::Fault(error.into()))),
    };
    let mut front = Vec::new();
    let writer = match ContainerWriter::begin(plan, &mut front) {
        Ok(writer) => writer,
        Err(error) => return Ok(Err(Stop::Fault(error.into()))),
    };
    if let Err(error) = file.write(&front).await {
        return Ok(Err(Stop::Fault(error.into())));
    }
    let mut carrying = Carrying::new(old, old_key, replacing.table, replacing.keep, writer);

    let mut buffer = vec![0u8; TRANSFER_BUFFER];
    let mut hasher = blake3::Hasher::new();
    let mut held: Option<Unverified> = None;
    loop {
        let read = reader.read(&mut buffer).await.map_err(Error::from)?;
        if read == 0 {
            break;
        }
        length.count(read)?;
        hasher.update(&buffer[..read]);
        if held.is_none() {
            match carrying.absorb(&buffer[..read], &mut file).await {
                Ok(()) => {}
                Err(Stop::Unverified(unverified)) => held = Some(unverified),
                Err(fault) => return Ok(Err(fault)),
            }
        }
    }
    length.finish()?;

    let actual = ContentHash::from_bytes(*hasher.finalize().as_bytes());
    if actual != old.ciphertext_hash {
        return Ok(Err(Unverified::CiphertextMismatch {
            expected: old.ciphertext_hash,
            actual,
        }
        .into()));
    }
    if let Some(unverified) = held {
        return Ok(Err(unverified.into()));
    }

    let entries = match carrying.finish(&mut file).await {
        Ok(entries) => entries,
        Err(stop) => return Ok(Err(stop)),
    };
    match file.finish().await {
        Ok(digests) => Ok(Ok((entries, digests))),
        Err(error) => Ok(Err(Stop::Fault(error.into()))),
    }
}

/// Removes what a refused rebuild left on this device: the spool, and then the
/// row naming it (spec: OC-2, OC-8).
///
/// Nothing was uploaded and no commit was attempted, so both are this device's
/// own and removing them is safe. The row goes only once the file has: a spool
/// that would not go stays named, and the next sync's settlement disposes of it
/// as it would any abandoned spool.
async fn dispose(
    with: &Rebuilding<'_>,
    container_id: ContainerId,
    spool_path: &Path,
) -> Result<(), RebuildError> {
    match with.spool.discard(spool_path).await {
        Ok(()) => with.index.clear_pending_row(container_id).await?,
        Err(error) => warn!(
            container = %container_id,
            reason = %error.redacted(),
            "a refused rebuild's spool file could not be removed; its row stays for the next sync",
        ),
    }
    Ok(())
}
