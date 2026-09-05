use std::collections::BTreeMap;

use coffret_format::{decode_index_snapshot, decode_journal_record, DecodedControlObject};
use coffret_model::{ControlObjectKind, ControlObjectName, Generation, Redacted, SnapshotContent};
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
    /// the commit slot is spent, and how a fetch reaches a Keyring replica or a
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
        // Which checkpoint this catalog came from is its own provenance and no
        // part of what a Snapshot carries, so it is stamped on here rather than
        // read out of the payload (spec: CK-7, CK-9).
        index.restore(content.adopted_from_object(object)).await?;
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
/// reported. Which of the two a refusal is, is [`skippable`]'s to say.
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
                let payload = decode_index_snapshot(&decoded.payload, decoded.kind, generation)
                    .map_err(|error| CommitError::CorruptControlObject {
                        object: name.clone(),
                        fault: ControlObjectFault::Unopenable(error),
                    })?;
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

/// Whether a refusal is evidence about one candidate rather than about the
/// Library or Storage.
///
/// The walk down the candidates only gets to try an older checkpoint if the
/// refusals it steps over are ones that say "not this object". Two do:
///
/// - the format layer refusing what arrived — it decrypted to nothing, its
///   header disagrees with its name, its payload is not the schema it claims;
/// - a declared length past the ceiling an object of that kind may be
///   ([`Error::ObjectTooLarge`]). It is the same finding one step earlier: a
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
fn skippable(error: &CommitError) -> bool {
    matches!(
        error,
        CommitError::Format(_) | CommitError::Storage(Error::ObjectTooLarge { .. })
    )
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
/// arrive at the same records. Nothing in the process settles that: a lock here
/// would not reach the other process, and the Index deliberately refuses a
/// record it already holds rather than absorbing it, because one Entry Path
/// admits one current Entry and one Container enters the current set once
/// (spec: EP-5, EP-6). So the convergence is this loop's, and it is convergence
/// and not tolerance — the checkpoint is what decides, in both directions.
///
/// Before each record the loop asks where the catalog stands. One the checkpoint
/// already covers is one somebody applied, and it is stepped over rather than
/// applied again — which matters most for the records that would *not* be
/// refused: a record carrying only removals is idempotent in everything except
/// the checkpoint it writes, so re-applying one would carry the catalog
/// backwards, and a replay only ever goes forwards (spec: CK-9).
///
/// What that reading cannot cover is the rival that commits in the instant
/// between it and the apply, and that arrives as the refusal. Then the
/// checkpoint is read again: if it now covers the record, the refusal means
/// "somebody else already did this" and the replay carries on. If it does not,
/// nothing explains the refusal and it is reported unchanged — a catalog that
/// genuinely holds two Entries at one path is not a race, and swallowing it
/// would hide the one failure this loop must not hide.
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
    let mut stepped_over = 0u64;
    for number in from..=newest_head.get() {
        let generation = Generation::new(number)
            .expect("a number no larger than the newest head is a generation");
        let decoded = match fetched.remove(&generation) {
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
                match index.apply(record).await {
                    Ok(()) => replayed += 1,
                    Err(refusal) => {
                        step_over(index, generation, refusal).await?;
                        stepped_over += 1;
                    }
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

/// Steps over a record another replayer applied first, and reports the refusal
/// where nothing explains it.
///
/// The only refusal another replayer accounts for is the collision a record
/// already in the catalog produces, and the only proof that this is what
/// happened is a checkpoint that has since moved past the record. Both have to
/// hold: a refusal of another shape says what it says whoever else is running,
/// and a duplicate over a catalog that stands where it did is a Library state no
/// commit could have produced (spec: EP-5, EP-6) rather than a race.
async fn step_over(
    index: &dyn Index,
    generation: Generation,
    refusal: IndexError,
) -> CommitResult<()> {
    if !is_already_held(&refusal) {
        return Err(refusal.into());
    }
    match index.checkpoint().await {
        Ok(Some(at)) if generation <= at.head_generation() => {
            debug!(
                generation = generation.get(),
                stands_at = at.head_generation().get(),
                "stepping over a record another replayer applied first",
            );
            Ok(())
        }
        Ok(_) => Err(refusal.into()),
        // Reading where the catalog stands is what would have explained the
        // refusal, and it did not answer — so the refusal stands, and why it
        // could not be explained is logged rather than reported in its place.
        Err(unreadable) => {
            warn!(
                generation = generation.get(),
                reason = %unreadable.redacted(),
                "could not read where the catalog stands after a refused replay",
            );
            Err(refusal.into())
        }
    }
}

/// Whether a refusal is the one a record the catalog already holds produces.
///
/// A record is taken in as its Containers and then their Entries, so a second
/// application of it collides on the first of the two: the Container is in the
/// current set already, or the path already holds the Entry it would put there.
/// Nothing else in the Index's vocabulary can mean that — an Entry naming no
/// Container, a catalog this build cannot read, a store that failed, all say the
/// same thing however many replayers there are.
fn is_already_held(refusal: &IndexError) -> bool {
    matches!(
        refusal,
        IndexError::DuplicateContainer { .. } | IndexError::DuplicatePath { .. }
    )
}
