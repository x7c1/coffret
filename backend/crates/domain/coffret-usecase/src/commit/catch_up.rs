use std::collections::BTreeMap;

use coffret_format::{decode_index_snapshot, decode_journal_record, DecodedControlObject};
use coffret_model::{ControlObjectKind, ControlObjectName, Generation, SnapshotContent};
use tracing::{debug, warn};

use crate::commit::commit_error::{CommitError, CommitResult, ControlObjectFault};
use crate::commit::control_keys::ControlKeys;
use crate::commit::control_listing::ControlListing;
use crate::commit::control_object;
use crate::control_head::ControlHead;
use crate::error::Error;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// What reading the Library's control state takes.
///
/// Four things travel together through every step of a catch-up — the store,
/// the keys to open what it holds, how long a call is worth retrying, and the
/// walk that says what is there — and none of them changes while it runs. They
/// are bundled rather than passed one by one because the two steps below take
/// the same four and would otherwise both be a list of arguments a reader has
/// to check against each other.
struct Reading<'a> {
    store: &'a dyn ObjectStore,
    keys: &'a ControlKeys,
    retry: &'a RetryPolicy,
    listing: &'a ControlListing,
}

impl Reading<'_> {
    /// Opens one control object the listing named.
    async fn open(&self, name: &ControlObjectName) -> CommitResult<Option<DecodedControlObject>> {
        let Some(object) = self.listing.handle(&name.to_string()) else {
            return Ok(None);
        };
        match control_object::read(self.store, self.retry, self.keys, name, object).await {
            Ok(decoded) => Ok(Some(decoded)),
            // Listed a moment ago and gone now: another device is running
            // `prune` or a rotation.
            Err(CommitError::Storage(Error::NotFound { .. })) => Ok(None),
            Err(error) => Err(error),
        }
    }
}

/// A Library read up to its head, and the Index standing there.
///
/// Visible to the crate because the catch-up is, and what it carries is the
/// commit flow's: a caller outside this module wants the Index at the head and
/// nothing else, so every field stays where the steps that use it are.
pub(crate) struct CaughtUp {
    /// The walk of Storage the catch-up read, kept for the steps that follow:
    /// the handles are how a removal is trashed and how the head is re-read
    /// before the commit slot is spent.
    pub(super) listing: ControlListing,
    /// The head the commit will succeed, or `None` in a Library that has
    /// committed nothing (spec: FM-13).
    pub(super) head: Option<ControlHead>,
    /// The newest checkpoint the Library holds, which is what the checkpoint
    /// policy counts the Journal from (spec: CK-8).
    pub(super) newest_checkpoint: Option<Generation>,
}

/// Brings the Index to the Library's current head (spec: CK-9).
///
/// The starting point is the newer of two: this Index, or the newest valid
/// checkpoint on Storage. When the checkpoint is newer its Library-wide content
/// is adopted and this device's own state is left alone; when the Index is
/// newer, as it usually is between Snapshots, it is kept. Either way the Journal
/// records after that point are replayed, and none of it opens a Container,
/// because a record carries what the Containers it adds hold (spec: CP-11).
///
/// This is also what a writer that lost the commit race runs before trying
/// again (spec: CP-4, EP-7), which is why it is one routine and not a preamble
/// to the happy path — and what a sync run calls on its own, before it scans, to
/// decide the fate of the pending rows an interrupted run left it (spec: OC-3,
/// OC-7). Nothing here writes to the Library, so a caller that only wants the
/// head read may stop here.
pub(crate) async fn catch_up(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    retry: &RetryPolicy,
) -> CommitResult<CaughtUp> {
    let listing = ControlListing::read(store, retry).await?;
    let newest_head = listing.newest_head();
    let mut newest_checkpoint = listing.newest_snapshot();
    let held = index.checkpoint().await?.map(|at| at.head_generation);
    let reading = Reading {
        store,
        keys,
        retry,
        listing: &listing,
    };

    // Heads fetched while looking for a starting point are exactly the ones the
    // replay below needs — the walk goes down from the newest head and the
    // replay comes back up from the checkpoint — so they are kept rather than
    // fetched twice.
    let mut fetched: BTreeMap<Generation, DecodedControlObject> = BTreeMap::new();
    let mut start = held;

    for generation in listing.checkpoint_candidates() {
        if held.is_some_and(|held| generation <= held) {
            // Everything below here is older than what this Index already
            // stands at, and the older of the two starting points is not the
            // one CK-9 takes.
            break;
        }
        let Some((object, content)) = adoptable(&reading, generation, &mut fetched).await? else {
            continue;
        };
        debug!(
            object = %object,
            generation = generation.get(),
            "adopting the newest checkpoint as the starting point",
        );
        index
            .restore(SnapshotContent {
                // Which checkpoint this catalog came from is its own provenance
                // and no part of what a Snapshot carries, so it is recorded
                // here rather than read out of the payload (spec: CK-7, CK-9).
                adopted_from: Some(object),
                ..content
            })
            .await?;
        start = Some(generation);
        newest_checkpoint = newest_checkpoint.max(Some(generation));
        break;
    }

    replay(&reading, index, start, newest_head, fetched).await?;

    Ok(CaughtUp {
        listing,
        head: newest_head.map(ControlHead::at),
        newest_checkpoint,
    })
}

/// The checkpoint at one generation, if there is a valid one there.
///
/// Both name forms are tried, because both kinds of object can be a checkpoint:
/// `idx-<generation>` always, `head-<generation>` when its header says
/// activation Snapshot (spec: CK-9, FM-12). A Journal record found at the head
/// name is not a checkpoint — it is kept for the replay and the walk carries on
/// downwards.
///
/// A candidate that does not open is skipped rather than reported: CK-9 takes
/// the newest *valid* checkpoint, so one that is corrupt leaves an older one
/// still usable. A Storage failure is not a verdict about validity and is
/// reported.
async fn adoptable(
    reading: &Reading<'_>,
    generation: Generation,
    fetched: &mut BTreeMap<Generation, DecodedControlObject>,
) -> CommitResult<Option<(ControlObjectName, SnapshotContent)>> {
    let mut names = Vec::with_capacity(2);
    if reading.listing.has_snapshot(generation) {
        names.push(ControlObjectName::index_snapshot(generation));
    }
    if reading.listing.has_head(generation) {
        names.push(ControlObjectName::head(generation));
    }

    for name in names {
        let spelling = name.to_string();
        let decoded = match reading.open(&name).await {
            Ok(Some(decoded)) => decoded,
            // Gone between the listing and the fetch: an object that is not
            // there is not a checkpoint this device can start from.
            Ok(None) => continue,
            Err(CommitError::Format(error)) => {
                warn!(
                    object = %spelling,
                    reason = %error,
                    "skipping a checkpoint candidate that did not open",
                );
                continue;
            }
            Err(error) => return Err(error),
        };
        match decoded.kind {
            ControlObjectKind::IndexSnapshot | ControlObjectKind::ActivationSnapshot => {
                let payload = decode_index_snapshot(&decoded.payload, decoded.kind)?;
                return Ok(Some((name, payload.content)));
            }
            // A head that is an ordinary commit: not a checkpoint, but exactly
            // what the replay is about to want.
            ControlObjectKind::Journal => {
                fetched.insert(generation, decoded);
            }
            ControlObjectKind::Keyring => {
                return Err(CommitError::CorruptControlObject {
                    object: name.clone(),
                    fault: ControlObjectFault::KindNotAdmitted {
                        found: decoded.kind,
                    },
                })
            }
        }
    }
    Ok(None)
}

/// Replays the Journal from just after `start` up to the newest head.
///
/// Only the records after the starting point, and every one of them: a gap is a
/// Library this device cannot catch up with rather than one it may commit into
/// (spec: CK-9).
async fn replay(
    reading: &Reading<'_>,
    index: &dyn Index,
    start: Option<Generation>,
    newest_head: Option<Generation>,
    mut fetched: BTreeMap<Generation, DecodedControlObject>,
) -> CommitResult<()> {
    let Some(newest_head) = newest_head else {
        return Ok(());
    };
    // The Library's first head is generation 0, so a device starting from
    // nothing replays from there (spec: FM-13).
    let from = match start {
        Some(generation) => generation.next()?.get(),
        None => Generation::FIRST.get(),
    };

    let mut replayed = 0u64;
    for number in from..=newest_head.get() {
        let generation = Generation::new(number);
        let decoded = match fetched.remove(&generation) {
            Some(decoded) => decoded,
            None => reading
                .open(&ControlObjectName::head(generation))
                .await?
                .ok_or(CommitError::MissingHead { generation })?,
        };
        match decoded.kind {
            ControlObjectKind::Journal => {
                index
                    .apply(decode_journal_record(&decoded.payload, generation)?)
                    .await?;
                replayed += 1;
            }
            // The slot this device would have committed into was taken by an
            // epoch activation, and what follows is sealed under a Master Key
            // it does not hold (spec: CP-5).
            ControlObjectKind::ActivationSnapshot => {
                return Err(CommitError::EpochActivated { generation })
            }
            ControlObjectKind::IndexSnapshot | ControlObjectKind::Keyring => {
                return Err(CommitError::CorruptControlObject {
                    object: ControlObjectName::head(generation),
                    fault: ControlObjectFault::KindNotAdmitted {
                        found: decoded.kind,
                    },
                })
            }
        }
    }

    if replayed > 0 {
        debug!(
            records = replayed,
            head = newest_head.get(),
            "replayed the Journal up to the current head",
        );
    }
    Ok(())
}
