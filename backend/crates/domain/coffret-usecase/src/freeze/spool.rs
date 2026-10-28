use std::path::Path;

use coffret_format::EntryPlan;
use coffret_model::{CiphertextLenClaim, ContainerKind, EntryNumber, EntryPath};
use tracing::debug;

use crate::commit::EntryRef;
use crate::device_state::{BatchId, DeviceTime};
use crate::freeze::freeze_error::{FreezeError, FreezeResult, SourceChange};
use crate::freeze::segment::Segment;
use crate::freeze::selected::Selected;
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::mapped_roots::MappedRoots;
use crate::pack_spool::{PackSpool, PackSpoolError};
use crate::spool::Spool;
use crate::spool_file::WRITE_CHUNK;
use crate::spooled_container::SpooledContainer;

/// Encodes one segment into a Pack and writes it to the spool, streaming.
///
/// The Pack is written by [`PackSpool`], which is where the order every spool
/// step keeps lives — the pending row before the file, `Spooled` once it is
/// flushed, the key wrapped after that (spec: OC-2, OC-3) — and where nothing
/// holds a Pack, or an Entry (spec: PK-5, PK-18). What is this step's own is
/// where the bytes come from: each member file, read a buffer at a time through
/// the mapped roots and handed straight on.
///
/// Every member's bytes are checked against the plan the scan drew as they pass
/// — a Pack whose entry table does not describe its own content is not something
/// to put on Storage — so a file that moved under the run stops it with
/// [`FreezeError::SourceChanged`] rather than being committed under a table that
/// lies about it.
///
/// Eight arguments, and none is one this step could derive: the catalog the row
/// is written into, the epoch's keys, both halves of the device's disk, where on
/// it the spool goes, and the three values one run supplies — its batch, its
/// clock, and the segment to pack.
#[allow(clippy::too_many_arguments)]
pub(super) async fn spool(
    index: &dyn Index,
    keys: &LibraryKeys,
    local: &dyn Spool,
    roots: &dyn MappedRoots,
    spool_dir: &Path,
    batch: &BatchId,
    now: DeviceTime,
    segment: &Segment<Selected>,
) -> FreezeResult<SpooledContainer> {
    let plans: Vec<EntryPlan> = segment
        .members
        .iter()
        .map(|member| member.plan.clone())
        .collect();
    // Built out of files on this device's own disk (spec: OC-7).
    let mut pack = PackSpool::begin(index, local, spool_dir, batch, now, &plans, true)
        .await
        .map_err(FreezeError::from)?;

    let mut buffer = vec![0u8; WRITE_CHUNK];
    for member in &segment.members {
        let mut reader = member.source.reader(roots).await?;
        let mut read = 0u64;
        loop {
            let filled = reader.read(&mut buffer).await?;
            if filled == 0 {
                break;
            }
            read += filled as u64;
            pack.write(&buffer[..filled])
                .await
                .map_err(|error| match error {
                    PackSpoolError::Format(error) => moved(error, &member.plan.path),
                    other => other.into(),
                })?;
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

    let written = pack
        .finish(index, keys)
        .await
        .map_err(|error| match error {
            PackSpoolError::Format(error) => closing(error, &plans),
            other => other.into(),
        })?;
    let digests = written.digests;

    debug!(
        container = %written.container_id,
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
        container_id: written.container_id,
        materializes: true,
        kind: ContainerKind::Pack,
        spool_path: written.spool_path,
        entries: written.entries,
        envelope: written.envelope,
        ciphertext_hash: digests.blake3,
        // The length measured off the object this device just wrote, held to
        // the bound the format puts on the number it will be written down as
        // (spec: FM-19).
        ciphertext_len: CiphertextLenClaim::new(digests.len)
            .map_err(coffret_format::Error::from)?,
        provider_digest: digests.md5,
        object_ref: None,
        announced_at: now,
        replaces: segment
            .members
            .iter()
            .filter_map(|member| member.absorbs)
            .collect(),
        // Each member is the Pack's Entry at its position (spec: FM-9), and one
        // it absorbs takes the place of that one-file Container's only Entry.
        supersedes: segment
            .members
            .iter()
            .enumerate()
            .filter_map(|(position, member)| Some((position, member.absorbs?)))
            .map(|(position, absorbed)| {
                Ok((
                    EntryNumber::at_position(position)?,
                    EntryRef::new(absorbed, EntryNumber::FIRST),
                ))
            })
            .collect::<coffret_model::Result<_>>()
            .map_err(coffret_format::Error::from)?,
        // An absorbed Entry whose content the file still holds keeps its
        // deposit time; every other member is a deposit (spec: PK-13, FM-15).
        carries: segment
            .members
            .iter()
            .enumerate()
            .filter_map(|(position, member)| Some((position, member.carries?)))
            .map(|(position, deposited_at)| Ok((EntryNumber::at_position(position)?, deposited_at)))
            .collect::<coffret_model::Result<_>>()
            .map_err(coffret_format::Error::from)?,
    })
}

impl From<PackSpoolError> for FreezeError {
    fn from(error: PackSpoolError) -> Self {
        match error {
            PackSpoolError::Index(error) => error.into(),
            PackSpoolError::Local(error) => error.into(),
            PackSpoolError::Format(error) => error.into(),
        }
    }
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
    use coffret_format::{unwrap_container_key, ContainerFootprint, ContainerOutline};
    use coffret_model::{ContainerId, ContentHash, DepositedAt, MasterKey, MasterKeyEpoch, Mtime};

    use super::*;
    use crate::entry_paths::entry_path;
    use crate::in_memory_fs::InMemoryFs;
    use crate::in_memory_index::InMemoryIndex;
    use crate::local_scan::SourceFile;
    use crate::MappedRelativeLocation;

    /// Where the fake spools, which is any path at all: nothing is on a disk.
    const SPOOL_DIR: &str = "/spool";

    /// Where the case's source files stand in the same fake.
    const FOLDER: &str = "/folder";

    /// CP-11, FM-9: what the Journal record says a Pack holds and what the Pack
    /// itself records are one table, because they are one object — the run
    /// takes the table off the encoder that wrote the meta section rather than
    /// walking the same plans a second time.
    ///
    /// A second walk is what this case exists to make impossible. Nothing about
    /// the two computations looks different at a glance, and a Library where
    /// they had drifted would answer every lookup from a record that places an
    /// Entry where the Container does not.
    #[tokio::test]
    async fn the_table_a_pack_records_is_the_one_its_encoder_wrote() {
        let local = InMemoryFs::new();
        // Three Entries of different lengths and one of no bytes at all, so the
        // table has offsets that only a walk of the whole segment produces —
        // and one row whose extent is empty, which is the row a walk that had
        // drifted by one Entry would place differently.
        let members: Vec<Selected> = [
            ("albums/a.jpg", &b"the first file's bytes"[..]),
            ("albums/b.jpg", &b""[..]),
            ("albums/c.jpg", &b"a third file, longer than the first"[..]),
        ]
        .into_iter()
        .map(|(path, content)| member(&local, path, content))
        .collect();
        let footprint = ContainerFootprint::of(
            ContainerKind::Pack,
            &members
                .iter()
                .map(|member| member.plan.clone())
                .collect::<Vec<_>>(),
        )
        .expect("a segment this size measures");

        let index = InMemoryIndex::new();
        local
            .prepare_dir(Path::new(SPOOL_DIR))
            .await
            .expect("preparing the spool directory must succeed");
        let keys = LibraryKeys::derive(
            &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
            MasterKeyEpoch::FIRST,
        );
        let spooled = spool(
            &index,
            &keys,
            &local,
            &local,
            Path::new(SPOOL_DIR),
            &BatchId::new("a-batch"),
            DeviceTime::from_unix_seconds(1_700_000_000),
            &Segment { members, footprint },
        )
        .await
        .expect("a Pack of three short files spools");

        let object = local
            .content(&spooled.spool_path)
            .expect("the spool file this run just wrote is there");
        let key = unwrap_container_key(
            keys.container_wrap(),
            &spooled.container_id,
            &spooled.envelope,
        )
        .expect("the envelope the run sealed opens under the same key");
        let outline = ContainerOutline::open(&object, &key).expect("the Pack the run wrote opens");

        assert_eq!(
            outline.entries(),
            spooled.entries.as_slice(),
            "the entry table the meta section carries is the one the run reported",
        );
    }

    /// PK-7, EP-7: a Pack names, by its own entry number, the one-file Entry
    /// each absorbed member takes the place of — the only Entry of the
    /// Container it is absorbed out of — and names nothing for a member that
    /// is a new file. That is what a rebase follows when another writer renamed
    /// an absorbed Entry before the Pack committed, so a number off by one
    /// would carry a rename to another file's content.
    #[tokio::test]
    async fn a_pack_names_the_entry_each_absorbed_member_takes_the_place_of() {
        let local = InMemoryFs::new();
        let absorbed_a = ContainerId::from_bytes([0x0a; ContainerId::BYTE_LEN]);
        let absorbed_c = ContainerId::from_bytes([0x0c; ContainerId::BYTE_LEN]);
        // The first absorbed file still holds its Entry's content, so the Pack
        // carries its deposit time; the second changed, so it is a deposit.
        let kept = DepositedAt::from_unix_seconds(1_600_000_000);
        let members: Vec<Selected> = [
            (
                "albums/a.jpg",
                &b"absorbed out of one Container"[..],
                Some(absorbed_a),
                Some(kept),
            ),
            (
                "albums/b.jpg",
                &b"a file new to the Library"[..],
                None,
                None,
            ),
            (
                "albums/c.jpg",
                &b"absorbed out of another"[..],
                Some(absorbed_c),
                None,
            ),
        ]
        .into_iter()
        .map(|(path, content, absorbs, carries)| Selected {
            absorbs,
            carries,
            ..member(&local, path, content)
        })
        .collect();
        let footprint = ContainerFootprint::of(
            ContainerKind::Pack,
            &members
                .iter()
                .map(|member| member.plan.clone())
                .collect::<Vec<_>>(),
        )
        .expect("a segment this size measures");

        let index = InMemoryIndex::new();
        local
            .prepare_dir(Path::new(SPOOL_DIR))
            .await
            .expect("preparing the spool directory must succeed");
        let keys = LibraryKeys::derive(
            &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
            MasterKeyEpoch::FIRST,
        );
        let spooled = spool(
            &index,
            &keys,
            &local,
            &local,
            Path::new(SPOOL_DIR),
            &BatchId::new("a-batch"),
            DeviceTime::from_unix_seconds(1_700_000_000),
            &Segment { members, footprint },
        )
        .await
        .expect("a Pack of three short files spools");

        let first = EntryNumber::FIRST;
        let third = EntryNumber::at_position(2).expect("a small position numbers");
        assert_eq!(
            spooled.supersedes.iter().collect::<Vec<_>>(),
            [
                (&first, &EntryRef::new(absorbed_a, EntryNumber::FIRST)),
                (&third, &EntryRef::new(absorbed_c, EntryNumber::FIRST)),
            ],
        );
        assert_eq!(spooled.entries[0].path, entry_path("albums/a.jpg"));
        assert_eq!(spooled.entries[2].path, entry_path("albums/c.jpg"));
        // PK-13, FM-15: only the member that carries an Entry's content
        // unchanged keeps a deposit time; the batch stamps the rest.
        assert_eq!(
            spooled.carries.into_iter().collect::<Vec<_>>(),
            [(first, kept)],
        );
    }

    /// One member of a segment, whose file the case plants in the fake.
    ///
    /// In the fake rather than on a real filesystem, for the reason the spool
    /// beside it is there: everything this step reads goes through
    /// [`MappedRoots`], so what the case needs is only that the files are where
    /// the scan would have found them.
    fn member(local: &InMemoryFs, path: &str, content: &[u8]) -> Selected {
        let local_path = Path::new(FOLDER).join(path);
        local.write_file(&local_path, content);
        let entry = entry_path(path);
        let mtime = Mtime::from_unix_seconds(1_700_000_000);
        Selected {
            source: SourceFile {
                path: entry.clone(),
                root: Path::new(FOLDER).to_path_buf(),
                relative: MappedRelativeLocation::from_entry_path(&entry_path(path)),
                size: content.len() as u64,
                mtime,
                btime: None,
            },
            plan: EntryPlan::new(
                entry,
                mtime,
                content.len() as u64,
                ContentHash::from_bytes(*blake3::hash(content).as_bytes()),
            ),
            absorbs: None,
            carries: None,
        }
    }

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
                moved(error, &entry_path("albums/a.jpg")),
                FreezeError::Format(_)
            ),
            "so the encode's own refusal is what comes back",
        );
    }
}
