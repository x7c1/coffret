use std::collections::BTreeMap;

use coffret_format::{decode_index_snapshot, decode_journal_record, DecodedControlObject};
use coffret_model::{
    ControlObjectKind, ControlObjectName, Generation, JournalRecord, Redacted, SnapshotContent,
};
use tracing::{debug, warn};

use crate::commit::commit_error::{CommitError, CommitResult, ControlObjectFault};
use crate::commit::control_keys::ControlKeys;
use crate::commit::control_listing::ControlListing;
use crate::commit::control_object;
use crate::control_head::ControlHead;
use crate::error::Error;
use crate::index::Index;
use crate::index_error::IndexError;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// The most decoded control objects one catch-up keeps from its walk
/// (spec: CK-12).
///
/// The walk goes down a stretch of Journal whose length nothing on this device
/// sets: the checkpoint policy's threshold is a trigger and not a bound, and a
/// Snapshot upload another device failed to land leaves the stretch growing
/// until the next one does (spec: CK-8). So the number of records the walk
/// passes is whatever the Library has, and the number it *holds* is this.
///
/// A few hundred is past the stretch the policy aims for and well inside what
/// one process can hold of records this small, which makes it the point where
/// the saving stops being worth its memory rather than a budget the ordinary
/// case is expected to reach.
pub(super) const MAX_HELD_RECORDS: usize = 256;

/// The most decoded payload bytes it keeps over that same walk (spec: CK-12).
///
/// The count above bounds the map; this bounds what is in it, because a record
/// is sized by the batch it committed and may be 256 MiB on its own
/// (spec: FM-11) — two hundred of those would be a bound in name only. At 64
/// MiB the whole walk costs what the format allows one Keyring replica, the
/// smallest of its control-object ceilings (spec: FM-11), so a catch-up adds
/// about one more object's worth rather than a multiple of the Library's
/// length. A single record past this on its own is simply never kept, and the
/// replay fetches it again.
const MAX_HELD_BYTES: u64 = 64 * 1024 * 1024;

/// What a catch-up's walk keeps for the replay that comes back up after it.
///
/// The pairing is the whole reason to keep anything: the walk goes down from
/// the newest head, the replay comes back up from the checkpoint, and the
/// records are the same ones — so holding one saves fetching it twice. What the
/// pairing does not come with is a bound, and both halves of the cost are set
/// elsewhere: how many records there are by the Library (spec: CK-8), how large
/// one may be by the format (spec: FM-11).
///
/// So the keeping is bounded and the walking is not: a record that would not
/// fit under either budget is passed over, and the replay reads that one from
/// Storage again — the same records, in the same order, to the same head
/// (spec: CK-12).
///
/// That sameness is what the rule obliges. The two budgets above are this
/// build's own memory choice, not numbers another implementation has to match:
/// one that held none of the walk and fetched every record twice would arrive
/// at the same head from the same starting point.
#[derive(Default)]
struct Held {
    records: BTreeMap<Generation, DecodedControlObject>,
    bytes: u64,
    passed_over: u64,
}

impl Held {
    /// Keeps one decoded record for the replay, where there is room for it.
    fn keep(&mut self, generation: Generation, decoded: DecodedControlObject) {
        let cost = decoded.payload.body.len() as u64;
        if self.records.len() >= MAX_HELD_RECORDS
            || self.bytes.saturating_add(cost) > MAX_HELD_BYTES
        {
            self.passed_over += 1;
            return;
        }
        self.bytes += cost;
        self.records.insert(generation, decoded);
    }

    /// Takes back the record at one generation, if the walk kept it.
    fn take(&mut self, generation: Generation) -> Option<DecodedControlObject> {
        self.records.remove(&generation)
    }
}

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
    /// the handles are how a removal is trashed, how the head is re-read before
    /// the commit slot is consumed, and how a fetch reaches a Keyring replica or a
    /// Container this device never uploaded and so holds no handle for
    /// (spec: FM-3, FM-12).
    pub(crate) listing: ControlListing,
    /// The head the commit will succeed, or `None` in a Library that has
    /// committed nothing (spec: FM-13).
    pub(super) head: Option<ControlHead>,
    /// The newest checkpoint the Library holds, which is what the checkpoint
    /// policy counts the Journal from (spec: CK-8).
    pub(super) newest_checkpoint: Option<Generation>,
}

impl CaughtUp {
    /// The generation of the head the catch-up reached, or `None` in a Library
    /// that has committed nothing (spec: FM-13).
    ///
    /// What a flow that reads the catalog after its catch-up prepares its
    /// batch against, and so where the batch's rebase starts from
    /// (spec: CP-4, EP-7).
    pub(crate) fn head_generation(&self) -> Option<Generation> {
        self.head.map(|head| head.generation())
    }
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
/// to the happy path — and what every use case runs before it reads the catalog
/// at all, a sync included: what the Index says about an Entry Path is an answer
/// about the Library only where it stands at the head, and a sync reads it both
/// to settle the pending rows an interrupted run left it (spec: OC-3, OC-7) and
/// to decide which local files are new. Nothing here writes to the Library, so a
/// caller that only wants the head read may stop here.
pub(crate) async fn catch_up(
    store: &dyn ObjectStore,
    index: &dyn Index,
    keys: &ControlKeys,
    retry: &RetryPolicy,
) -> CommitResult<CaughtUp> {
    let listing = ControlListing::read(store, retry).await?;
    let newest_head = listing.newest_head();
    let mut newest_checkpoint = listing.newest_snapshot();
    let held = index.checkpoint().await?.map(|at| at.head_generation());
    let reading = Reading {
        store,
        keys,
        retry,
        listing: &listing,
    };

    // Heads fetched while looking for a starting point are exactly the ones the
    // replay below needs, so they are kept rather than fetched twice, up to what
    // one catch-up holds (spec: CK-12).
    let mut fetched = Held::default();
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
        // Which checkpoint this catalog came from is its own provenance and no
        // part of what a Snapshot carries, so it is stamped on here rather than
        // read out of the payload (spec: CK-7, CK-9).
        index.restore(content.adopted_from_object(object)).await?;
        start = Some(generation);
        newest_checkpoint = newest_checkpoint.max(Some(generation));
        break;
    }

    if fetched.passed_over > 0 {
        debug!(
            held = fetched.records.len(),
            fetched_again = fetched.passed_over,
            "the walk passed more records than one catch-up holds, and the replay \
             reads the rest again",
        );
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
/// reported. Which of the two a refusal is, is [`skippable`]'s to say.
///
/// A Snapshot from before entry numbers is passed over too, though it opens: it
/// is a sound object, only not one a catalog may start from (spec: FM-16).
///
/// That skipping covers a candidate this device could not open at all. One that
/// opens and then carries a payload no writer could have written — a Snapshot
/// checkpointing a head other than the one its name is for, say — is reported
/// instead: the object arrived whole and is still not the control object the
/// Library names, which is a verdict about this Library rather than something
/// an older checkpoint answers.
async fn adoptable(
    reading: &Reading<'_>,
    generation: Generation,
    fetched: &mut Held,
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
            Err(error) if skippable(&error) => {
                warn!(
                    object = %spelling,
                    reason = %error.redacted(),
                    "skipping a checkpoint candidate that is not one to start from",
                );
                continue;
            }
            Err(error) => return Err(error),
        };
        match decoded.kind {
            ControlObjectKind::IndexSnapshot | ControlObjectKind::ActivationSnapshot => {
                // The name's own generation goes to the decoder, which is where
                // the rule that a Snapshot checkpoints the head it is named for
                // is held (spec: CK-10): a candidate that checkpoints another
                // head is not one this device may start from.
                let payload =
                    match decode_index_snapshot(&decoded.payload, decoded.kind, generation) {
                        Ok(payload) => payload,
                        // Sound, but from before entry numbers: its Entries carry no
                        // position in their Container, and nothing here may make one
                        // up from path order. So it is no starting point, and the
                        // walk goes on down — to an older checkpoint that has them,
                        // or to none, and a replay of every record from `head-0`,
                        // which numbers each Entry by its place in its addition
                        // (spec: FM-16).
                        Err(coffret_format::Error::SnapshotPredatesEntryNumbers { schema }) => {
                            debug!(
                                object = %spelling,
                                schema,
                                "passing over a checkpoint that predates entry numbers",
                            );
                            continue;
                        }
                        Err(error) => {
                            return Err(CommitError::CorruptControlObject {
                                object: name.clone(),
                                fault: ControlObjectFault::Unopenable(error),
                            })
                        }
                    };
                return Ok(Some((name, payload.content)));
            }
            // A head that is an ordinary commit: not a checkpoint, but exactly
            // what the replay is about to want.
            ControlObjectKind::Journal => {
                fetched.keep(generation, decoded);
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

/// Whether a refusal is evidence about one candidate rather than about the
/// Library or Storage.
///
/// The walk down the candidates only gets to try an older checkpoint if the
/// refusals it steps over are ones that say "not this object". Two do:
///
/// - the format layer refusing what arrived — it decrypted to nothing, its
///   header disagrees with its name, its payload is not the schema it claims;
/// - a declared length past the ceiling an object of that kind may be
///   ([`Error::ObjectTooLong`]). It is the same finding one step earlier: a
///   size no Library produces is a lie about that object, told before the tag
///   that would have caught it could be reached. Refusing to read it is the
///   whole point of the ceiling, and reporting the refusal instead of stepping
///   over it would let one object anybody with write access can create leave
///   this device permanently unable to catch up — which the ceiling would then
///   have turned a bounded allocation into (spec: CK-9, RV-1).
///
/// Everything else is Storage having a bad minute, a catalog that would not
/// take the content, or a Library state no commit could have produced, and
/// none of those is answered by trying an older checkpoint.
///
/// One refusal of the format layer's is among the everything else: a length
/// this build cannot address. It is this build refusing, not the object being
/// wrong — a 64-bit build reads what a 32-bit one declines — so stepping over
/// it would be walking back past a head the Library really holds, on the
/// strength of a limit that is this device's own.
fn skippable(error: &CommitError) -> bool {
    match error {
        CommitError::Format(coffret_format::Error::UnaddressableOnThisBuild { .. }) => false,
        CommitError::Format(_) | CommitError::Storage(Error::ObjectTooLong { .. }) => true,
        _ => false,
    }
}

/// Replays the Journal from just after `start` up to the newest head.
///
/// Only the records after the starting point, and every one of them: a gap is a
/// Library this device cannot catch up with rather than one it may commit into
/// (spec: CK-9).
///
/// # Two replayers over one catalog
///
/// This is not the only catch-up that may be running against this Index. The
/// same catalog file is open in every process that holds the Library — a server
/// answering a browser while a `sync` runs in a terminal — and each of them
/// listed the Journal from its own reading of the checkpoint, so two of them
/// arrive at the same records. Nothing in the process prevents that: a lock here
/// would not reach the other process. So the convergence is this loop's, and the
/// checkpoint is what decides it.
///
/// Before each record the loop asks where the catalog stands, and one the
/// checkpoint already covers is stepped over rather than handed to the Index.
/// What that reading cannot cover is the rival that commits in the instant
/// between it and the apply. The Index closes that window itself: it refuses a
/// record at or below the head it stands at with
/// [`IndexError::AlreadyApplied`], inside the same operation and changing
/// nothing (spec: CK-9, RV-5), and that refusal is stepped over too. Every
/// other refusal is reported unchanged — a catalog that genuinely holds two
/// Entries at one path, or a rename no state rule admits, is not a race, and
/// swallowing it would hide the one failure this loop must not hide.
async fn replay(
    reading: &Reading<'_>,
    index: &dyn Index,
    start: Option<Generation>,
    newest_head: Option<Generation>,
    mut fetched: Held,
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
    let mut stepped_over = 0u64;
    for number in from..=newest_head.get() {
        let generation = Generation::new(number)
            .expect("a number no larger than the newest head is a generation");
        let decoded = match fetched.take(generation) {
            Some(decoded) => decoded,
            None => reading
                .open(&ControlObjectName::head(generation))
                .await?
                .ok_or(CommitError::MissingHead { generation })?,
        };
        match decoded.kind {
            ControlObjectKind::Journal => {
                if covered_by_checkpoint(index, generation).await? {
                    stepped_over += 1;
                    continue;
                }
                let record = decode_journal_record(&decoded.payload, generation)?;
                match replay_record(index, record).await? {
                    Replayed::Applied => replayed += 1,
                    Replayed::AlreadyApplied => stepped_over += 1,
                }
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

    if replayed > 0 || stepped_over > 0 {
        debug!(
            records = replayed,
            // Never a number a healthy single replay reaches: anything above
            // zero says another replayer was working on this catalog at the
            // same time, which is the one thing a reader of this line cannot
            // infer from anywhere else.
            already_held = stepped_over,
            head = newest_head.get(),
            "replayed the Journal up to the current head",
        );
    }
    Ok(())
}

/// Reads the Journal records of every generation in `generations`, in order,
/// off the listing a catch-up took.
///
/// What a rebase examines: the records committed after the head a batch was
/// prepared against, up to the head the catch-up reached (spec: CP-4, EP-7).
/// They are read here rather than taken from the replay, because the replay
/// does not see every one of them — another process may have applied them to
/// the shared catalog first, or a Snapshot covering them may have been
/// adopted (spec: CK-9).
///
/// Every generation has to hold a Journal record: a gap is the
/// [`CommitError::MissingHead`] a replay meets, and an activation is the
/// [`CommitError::EpochActivated`] it stops at (spec: CP-5).
pub(super) async fn read_records(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    retry: &RetryPolicy,
    listing: &ControlListing,
    generations: impl IntoIterator<Item = Generation>,
) -> CommitResult<Vec<JournalRecord>> {
    let reading = Reading {
        store,
        keys,
        retry,
        listing,
    };
    let mut records = Vec::new();
    for generation in generations {
        let name = ControlObjectName::head(generation);
        let decoded = reading
            .open(&name)
            .await?
            .ok_or(CommitError::MissingHead { generation })?;
        match decoded.kind {
            ControlObjectKind::Journal => {
                records.push(decode_journal_record(&decoded.payload, generation)?);
            }
            ControlObjectKind::ActivationSnapshot => {
                return Err(CommitError::EpochActivated { generation })
            }
            ControlObjectKind::IndexSnapshot | ControlObjectKind::Keyring => {
                return Err(CommitError::CorruptControlObject {
                    object: name,
                    fault: ControlObjectFault::KindNotAdmitted {
                        found: decoded.kind,
                    },
                })
            }
        }
    }
    Ok(records)
}

/// Whether the catalog already stands at or past one record's head.
///
/// The checkpoint is a head generation and a replay is forward-only, so a record
/// at or below it is one the catalog has taken in — from this loop, from another
/// process replaying the same Journal, or from a Snapshot adopted at or after it
/// (spec: CK-9).
async fn covered_by_checkpoint(index: &dyn Index, generation: Generation) -> CommitResult<bool> {
    Ok(index
        .checkpoint()
        .await?
        .is_some_and(|at| generation <= at.head_generation()))
}

/// What replaying one record came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Replayed {
    /// The catalog took it in.
    Applied,
    /// The catalog already stood at or past it, and is left as it was.
    AlreadyApplied,
}

/// Hands one record to the Index, stepping over it where another replayer
/// applied it first.
///
/// [`IndexError::AlreadyApplied`] is the one refusal another replayer accounts
/// for: the Index raises it from its own checkpoint, inside the operation that
/// would otherwise have applied the record, and changes nothing when it does
/// (spec: CK-9, RV-5). Every other refusal says what it says whoever else is
/// running, and is reported.
async fn replay_record(index: &dyn Index, record: JournalRecord) -> CommitResult<Replayed> {
    match index.apply(record).await {
        Ok(()) => Ok(Replayed::Applied),
        Err(IndexError::AlreadyApplied {
            generation,
            checkpoint,
        }) => {
            debug!(
                generation = generation.get(),
                stands_at = checkpoint.get(),
                "stepping over a record another replayer applied first",
            );
            Ok(Replayed::AlreadyApplied)
        }
        Err(refusal) => Err(refusal.into()),
    }
}

#[cfg(test)]
mod tests {
    use coffret_format::ControlPayload;
    use coffret_model::{
        CiphertextLenClaim, ContainerAddition, ContainerId, ContainerKind, ContainerSummary,
        ContentHash, EntryExtent, EntryMetadata, EntryNumber, EntryPath, EntryRename,
        MasterKeyEpoch, Mtime, ReplicaPosition,
    };

    use super::*;
    use crate::commit::control_fixtures::commitment;
    use crate::device_state::{DeviceTime, LocalObservation};
    use crate::entry_paths::entry_path;
    use crate::generations::generation;
    use crate::in_memory_index::InMemoryIndex;
    use crate::index_error::RenameRule;

    /// One decoded Journal record whose payload weighs `bytes`.
    ///
    /// The bytes are never read, so they are never written either: what a case
    /// here is about is the accounting, and the pages behind a zeroed vector
    /// stay unmapped until somebody touches them.
    fn record_of(bytes: usize) -> DecodedControlObject {
        DecodedControlObject {
            kind: ControlObjectKind::Journal,
            generation: Generation::FIRST,
            replica: ReplicaPosition::SINGLE,
            payload: ControlPayload::new(MasterKeyEpoch::FIRST, vec![0; bytes]),
        }
    }

    // CK-12: the count is not the whole of the bound, because one record is
    // sized by the batch that committed it and may be 256 MiB on its own
    // (spec: FM-11) — a few hundred of those would be a bound in name only. So
    // what the walk holds is bounded in bytes as well, and the record that
    // would pass the budget is left for the replay to fetch again rather than
    // half-kept.
    #[test]
    fn a_walk_stops_keeping_once_the_records_come_to_too_much() {
        let half = (MAX_HELD_BYTES / 2) as usize + 1;
        let mut held = Held::default();

        held.keep(generation(1), record_of(half));
        held.keep(generation(2), record_of(half));

        assert_eq!(
            held.records.len(),
            1,
            "two records past the budget between them cost one of them its place",
        );
        assert_eq!(held.passed_over, 1);
        assert!(
            held.take(generation(2)).is_none(),
            "and the one passed over is the one the replay reads again",
        );
    }

    // The same budget, met by one record rather than by a stretch of them: a
    // record larger than the whole of what a walk holds is never held at any
    // point in one, so a Library with an oversized record in it does not turn
    // every catch-up into an allocation the size of that record.
    #[test]
    fn a_record_larger_than_the_whole_budget_is_never_kept() {
        let mut held = Held::default();

        held.keep(generation(1), record_of(MAX_HELD_BYTES as usize + 1));

        assert!(held.records.is_empty(), "nothing was kept for it");
        assert_eq!(
            held.bytes, 0,
            "and nothing was accounted against the budget"
        );
    }

    // The count budget, at the boundary: what the walk holds is the bound, and
    // the records past it are the ones the replay reads a second time
    // (spec: CK-12).
    #[test]
    fn a_walk_longer_than_the_count_keeps_the_count_and_no_more() {
        let mut held = Held::default();

        for number in 1..=MAX_HELD_RECORDS as u64 + 4 {
            held.keep(generation(number), record_of(8));
        }

        assert_eq!(held.records.len(), MAX_HELD_RECORDS);
        assert_eq!(held.passed_over, 4);
    }

    // CK-9: the walk steps back past an object that is not what it claims,
    // and never past a length this build cannot address. That one says
    // nothing about the object — a 64-bit build reads it — so it is reported
    // rather than stepped over.
    #[test]
    fn a_length_this_build_cannot_address_is_not_stepped_over() {
        let unaddressable = CommitError::Format(coffret_format::Error::UnaddressableOnThisBuild {
            what: "control object",
            declared: u64::MAX,
        });
        assert!(!skippable(&unaddressable));

        let corrupt = CommitError::Format(coffret_format::Error::AuthenticationFailed);
        assert!(skippable(&corrupt));
    }

    /// A Library holding one Entry per Container, `inbox/a.jpg` in the first
    /// and `inbox/b.jpg` in the second, at its first head.
    fn two_files() -> JournalRecord {
        let one_file = |seed: u8, text: &str| {
            ContainerAddition::new(
                ContainerSummary {
                    id: ContainerId::from_bytes([seed; ContainerId::BYTE_LEN]),
                    kind: ContainerKind::OneFile,
                    ciphertext_hash: ContentHash::from_bytes([seed; ContentHash::BYTE_LEN]),
                    ciphertext_len: CiphertextLenClaim::new(128).expect("a literal length"),
                    object_ref: None,
                },
                vec![EntryMetadata {
                    path: entry_path(text),
                    captured_path: entry_path(text),
                    extent: EntryExtent::new(0, 10).expect("a literal extent"),
                    mtime: Mtime::from_unix_seconds(1_700_000_000),
                    btime: None,
                    hash: ContentHash::from_bytes([seed; ContentHash::BYTE_LEN]),
                    derived_from: None,
                    mime: None,
                }],
            )
            .expect("a one-file table tiles")
        };
        JournalRecord::new(
            Generation::FIRST,
            None,
            MasterKeyEpoch::FIRST,
            commitment(),
            None,
            None,
            vec![one_file(0x21, "inbox/a.jpg"), one_file(0x31, "inbox/b.jpg")],
            Vec::new(),
        )
        .expect("the first head succeeds nothing")
    }

    /// The record at `number` renaming the only Entry of the Container of each
    /// seed in `renames` to the path beside it, in that order.
    fn renaming(number: u64, renames: &[(u8, &str)]) -> JournalRecord {
        JournalRecord::new(
            generation(number),
            Some(generation(number - 1)),
            MasterKeyEpoch::FIRST,
            commitment(),
            None,
            None,
            Vec::new(),
            Vec::new(),
        )
        .and_then(|record| {
            record.with_renames(
                renames
                    .iter()
                    .map(|(seed, to)| {
                        EntryRename::new(
                            ContainerId::from_bytes([*seed; ContainerId::BYTE_LEN]),
                            EntryNumber::FIRST,
                            entry_path(*to),
                        )
                    })
                    .collect::<Result<_, _>>()?,
            )
        })
        .expect("a record renaming Entries in canonical order")
    }

    // CK-9, RV-5, FM-15: two replayers over one catalog, the slower of which
    // read the checkpoint before the faster applied the record it then hands
    // the Index. No state rule refuses either record the second time — the
    // swap would swap its two Entries back, and the rename of an Entry a later
    // record renamed again would carry it back to where it was — so it is the
    // Index's refusal by generation that the slower one steps over, and the
    // catalog and the files waiting to follow their Entries stay where the
    // faster one left them.
    #[tokio::test]
    async fn the_slower_of_two_replayers_steps_over_what_the_faster_applied() {
        let index = InMemoryIndex::new();
        index
            .apply(two_files())
            .await
            .expect("the first head applies");
        for (text, seed) in [("inbox/a.jpg", 0x21), ("inbox/b.jpg", 0x31)] {
            index
                .mark_present(LocalObservation {
                    path: entry_path(text),
                    size: 10,
                    mtime: Mtime::from_unix_seconds(1_700_000_000),
                    at: DeviceTime::from_unix_seconds(1_700_000_500),
                    hash: Some(ContentHash::from_bytes([seed; ContentHash::BYTE_LEN])),
                })
                .await
                .expect("recording a materialized file must succeed");
        }
        let swap = renaming(1, &[(0x21, "inbox/b.jpg"), (0x31, "inbox/a.jpg")]);
        let rename = renaming(2, &[(0x21, "albums/a.jpg")]);
        let rename_again = renaming(3, &[(0x21, "albums/z.jpg")]);

        for (raced, ahead) in [
            (swap.clone(), vec![swap]),
            (rename.clone(), vec![rename, rename_again]),
        ] {
            // The slower replayer reads where the catalog stands first ...
            assert!(!covered_by_checkpoint(&index, raced.generation())
                .await
                .expect("reading the checkpoint must succeed"));
            // ... and the faster one replays ahead of it before it gets on.
            for record in ahead {
                assert_eq!(
                    replay_record(&index, record)
                        .await
                        .expect("the faster replayer applies each record"),
                    Replayed::Applied,
                );
            }
            let stood_at = index
                .snapshot()
                .await
                .expect("an applied catalog has a state to checkpoint");
            let waited_for = waiting(&index).await;

            assert_eq!(
                replay_record(&index, raced)
                    .await
                    .expect("a record another replayer applied first is not a failure"),
                Replayed::AlreadyApplied,
            );
            assert_eq!(
                index
                    .snapshot()
                    .await
                    .expect("an applied catalog has a state to checkpoint"),
                stood_at,
                "the slower replay does not carry the catalog back",
            );
            assert_eq!(waiting(&index).await, waited_for);
        }
        assert_eq!(
            waiting(&index).await,
            [
                Some(entry_path("albums/z.jpg")),
                Some(entry_path("inbox/a.jpg"))
            ],
            "each file waits for where its Entry stands now",
        );
    }

    /// Where the files at `inbox/a.jpg` and `inbox/b.jpg` wait to follow
    /// their Entries to.
    async fn waiting(index: &InMemoryIndex) -> [Option<EntryPath>; 2] {
        let mut waiting = [None, None];
        for (slot, text) in waiting.iter_mut().zip(["inbox/a.jpg", "inbox/b.jpg"]) {
            *slot = index
                .local_entry_at(&entry_path(text))
                .await
                .expect("reading a local file's row must succeed")
                .expect("this device keeps a row for each file")
                .relocate_to;
        }
        waiting
    }

    // CK-9, FM-15: a rename the catalog refuses by a state rule is not one
    // somebody else applied — it is reported as it stands.
    #[tokio::test]
    async fn a_refused_rename_is_reported_as_it_stands() {
        let index = InMemoryIndex::new();
        index
            .apply(two_files())
            .await
            .expect("the first head applies");

        match replay_record(&index, renaming(1, &[(0x21, "inbox/a.jpg")])).await {
            Err(CommitError::Index(IndexError::RenameRefused {
                element: 0,
                rule: RenameRule::PathUnchanged,
            })) => {}
            other => panic!("expected the refusal to be reported as it stands, got {other:?}"),
        }
    }
}
