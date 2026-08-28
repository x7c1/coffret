use std::path::Path;

use coffret_format::{
    generate_container_id, generate_container_key, wrap_container_key, ContainerWriter, EncodePlan,
    EntryPlan,
};
use coffret_model::{ContainerKind, EntryMetadata, EntryPath};
use tracing::debug;

use crate::device_state::{BatchId, DeviceTime, PendingUpload, SpoolState};
use crate::freeze::freeze_error::{FreezeError, FreezeResult, SourceChange};
use crate::freeze::segment::Segment;
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::spool_file::{SpoolFile, WRITE_CHUNK};
use crate::spooled_container::SpooledContainer;

/// Encodes one segment into a Pack and writes it to the spool, streaming.
///
/// The order is the sync's, and for the same reason: the pending row is recorded
/// before the spool file is created, so a run that dies anywhere in the write —
/// or is killed without an error path at all — leaves a row naming what it may
/// have put on disk, the positive local provenance that makes cleaning it up
/// possible at all (spec: OC-2, OC-3). The row is flipped to
/// [`Spooled`](crate::device_state::SpoolState::Spooled) once the Pack is
/// flushed, and the Container Key is wrapped after that, so a wrap failure
/// leaves a complete spool a row already calls `Spooled`.
///
/// What is not the sync's is the shape of the encode. A Pack is around a
/// gigabyte and an oversized singleton is whatever one indivisible Entry happens
/// to be (spec: PK-3, PK-5), so nothing here holds a Pack, or even an Entry: the
/// scan settled the entry table, so [`ContainerWriter`] writes the header and
/// the table at once and then takes each member file a buffer at a time,
/// handing back ciphertext that goes straight to the spool. The two digests are
/// folded in as those bytes hit disk. What the run holds is one read buffer and
/// one sink buffer, whatever the Pack weighs.
///
/// Every member's bytes are checked against the plan the scan drew as they pass
/// — a Pack whose entry table does not describe its own content is not something
/// to put on Storage — so a file that moved under the run stops it with
/// [`FreezeError::SourceChanged`] rather than being committed under a table that
/// lies about it.
pub(super) async fn spool(
    index: &dyn Index,
    keys: &LibraryKeys,
    spool_dir: &Path,
    batch: &BatchId,
    now: DeviceTime,
    segment: &Segment,
) -> FreezeResult<SpooledContainer> {
    let container_id = generate_container_id()?;
    let container_key = generate_container_key()?;
    let plans: Vec<EntryPlan> = segment
        .members
        .iter()
        .map(|member| member.plan.clone())
        .collect();

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

    // Drained into the spool after every step, so what the run holds is one
    // chunk of ciphertext rather than a Pack of it.
    let mut sink = Vec::with_capacity(WRITE_CHUNK);
    let mut writer = ContainerWriter::begin(
        &EncodePlan {
            container_id,
            kind: ContainerKind::Pack,
            key: &container_key,
            chunk_size: coffret_format::ChunkSize::DEFAULT,
            entries: &plans,
        },
        &mut sink,
    )?;
    spool.write(&sink).await?;
    sink.clear();

    let mut buffer = vec![0u8; WRITE_CHUNK];
    for member in &segment.members {
        let mut reader = member.source.open().await?;
        let mut read = 0u64;
        loop {
            let filled = reader.read(&mut buffer).await?;
            if filled == 0 {
                break;
            }
            read += filled as u64;
            writer
                .write(&buffer[..filled], &mut sink)
                .map_err(|error| moved(error, &member.plan.path))?;
            // Drained here rather than after the file, so what the run holds
            // stays one buffer whatever one Entry weighs.
            spool.write(&sink).await?;
            sink.clear();
        }
        if read != member.plan.size {
            // The one detection site with no format error in hand: the encoder
            // is still waiting for the rest of the Entry, and what says the
            // file is short is this run's own count of what it handed over.
            return Err(FreezeError::SourceChanged {
                path: member.plan.path.clone(),
                cause: SourceChange::LengthMoved {
                    expected: member.plan.size,
                    actual: read,
                },
            });
        }
    }

    writer
        .finish(&mut sink)
        .map_err(|error| closing(error, &plans))?;
    spool.write(&sink).await?;
    let digests = spool.finish().await?;
    index.mark_spooled(container_id).await?;

    let envelope = wrap_container_key(keys.container_wrap(), &container_id, &container_key)?;

    debug!(
        container = %container_id,
        entries = plans.len(),
        footprint = segment.footprint.bytes(),
        bytes = digests.len,
        absorbs = segment
            .members
            .iter()
            .filter(|member| member.absorbs.is_some())
            .count(),
        "encoded a Pack and spooled it",
    );
    Ok(SpooledContainer {
        container_id,
        kind: ContainerKind::Pack,
        spool_path,
        entries: entry_table(&plans),
        envelope,
        ciphertext_hash: digests.blake3,
        ciphertext_len: digests.len,
        provider_digest: digests.md5,
        object_ref: None,
        replaces: segment
            .members
            .iter()
            .filter_map(|member| member.absorbs)
            .collect(),
    })
}

/// What the Journal record says the Pack holds (spec: CP-11, FM-9).
///
/// The offsets are the ones the encoder assigned, which is the same walk it
/// makes: every Entry lands after the one before it, so the table describes the
/// stream that was written next to it.
fn entry_table(plans: &[EntryPlan]) -> Vec<EntryMetadata> {
    let mut offset = 0u64;
    plans
        .iter()
        .map(|plan| {
            let entry = EntryMetadata {
                path: plan.path.clone(),
                offset,
                size: plan.size,
                mtime: plan.mtime,
                hash: plan.hash,
                derived_from: plan.derived_from.clone(),
                mime: plan.mime.clone(),
            };
            offset += plan.size;
            entry
        })
        .collect()
}

/// What a refused encode of one member means.
///
/// The encoder holds the content to the entry table the scan drew, and the ways
/// it can disagree — a length that moved, a hash that moved, more bytes than the
/// table plans for — are one verdict from here: the file is no longer the file
/// the scan measured. Which of them it was is not, so it comes along as a
/// [`SourceChange`] rather than being flattened away. Everything else the format
/// layer reports travels unchanged.
fn moved(error: coffret_format::Error, path: &EntryPath) -> FreezeError {
    match change(&error) {
        Some(cause) => FreezeError::SourceChanged {
            path: path.clone(),
            cause,
        },
        None => FreezeError::Format(error),
    }
}

/// The same verdict for a Pack refused as it closes, where the entry table is
/// what names the file the writer is still waiting on.
///
/// Only the two refusals that carry an index reach it — a close has no member in
/// hand, so the entry table is the only thing that can name one — and the verdict
/// itself is [`moved`]'s.
fn closing(error: coffret_format::Error, plans: &[EntryPlan]) -> FreezeError {
    let index = match error {
        coffret_format::Error::EntryLengthMismatch { index, .. }
        | coffret_format::Error::EntryHashMismatch { index } => index,
        _ => return FreezeError::Format(error),
    };
    moved(error, &plans[index].path)
}

/// What the encoder's refusal says moved, if it says a file moved at all.
///
/// The three refusals are read once, here, so that a Pack refused mid-stream and
/// one refused as it closes cannot come to read them differently.
fn change(error: &coffret_format::Error) -> Option<SourceChange> {
    match error {
        coffret_format::Error::EntryLengthMismatch {
            expected, actual, ..
        } => Some(SourceChange::LengthMoved {
            expected: *expected,
            actual: *actual,
        }),
        coffret_format::Error::EntryHashMismatch { .. } => Some(SourceChange::ContentMoved),
        coffret_format::Error::StreamOverrun { planned } => {
            Some(SourceChange::GrewPastTheTable { planned: *planned })
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reading a person gets is only as good as which of the encoder's
    /// numbers ends up in which field, and a run reaches this mapping through
    /// the encoder rather than through anything a case can plant, so the three
    /// refusals are read here directly.
    #[test]
    fn a_length_that_moved_carries_both_lengths_the_right_way_round() {
        let cause = change(&coffret_format::Error::EntryLengthMismatch {
            index: 2,
            expected: 180,
            actual: 100,
        });
        let Some(SourceChange::LengthMoved { expected, actual }) = cause else {
            panic!("a length that moved is a length that moved, got {cause:?}");
        };
        assert_eq!(expected, 180, "the length the entry table records");
        assert_eq!(actual, 100, "and the length that arrived");
    }

    #[test]
    fn a_hash_that_moved_is_content_and_not_length() {
        let cause = change(&coffret_format::Error::EntryHashMismatch { index: 0 });
        assert!(
            matches!(cause, Some(SourceChange::ContentMoved)),
            "a file that kept its length and not its bytes was rewritten, got {cause:?}",
        );
    }

    #[test]
    fn an_overrun_carries_the_plan_it_passed() {
        let cause = change(&coffret_format::Error::StreamOverrun { planned: 512 });
        let Some(SourceChange::GrewPastTheTable { planned }) = cause else {
            panic!("an overrun is a file that grew past the table, got {cause:?}");
        };
        assert_eq!(planned, 512, "what the whole entry table plans for");
    }

    /// Everything else the format layer reports is not a verdict about the
    /// local file, so it travels unchanged rather than being read as one.
    #[test]
    fn any_other_refusal_says_nothing_about_the_file() {
        let error = coffret_format::Error::AuthenticationFailed;
        assert!(change(&error).is_none(), "not a file that moved");
        assert!(
            matches!(
                moved(error, &EntryPath::nfc("albums/a.jpg")),
                FreezeError::Format(_)
            ),
            "so the encode's own refusal is what comes back",
        );
    }
}
