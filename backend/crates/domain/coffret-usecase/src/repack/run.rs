use std::collections::{BTreeMap, BTreeSet};

use coffret_format::EntryPlan;
use coffret_model::{
    CiphertextLenClaim, ContainerId, ContainerKeyStatus, ContainerKind, EntryNumber, KeyEnvelope,
    KeyTable, Redacted,
};
use tracing::{debug, info, warn};

use crate::commit::{catch_up, read_committed, EntryRef, ShortReport};
use crate::delete::PackRefusal;
use crate::device_state::BatchId;
use crate::freeze::segment::Segment;
use crate::pack_spool::PackSpool;
use crate::progress::{Phase, Step};
use crate::read_modify_replace::{rebuild, Rebuilding, Replacing};
use crate::repack::plan::{self, Cut, Held, Member, Plan};
use crate::repack::repack_error::{RepackError, RepackResult};
use crate::repack::repack_outcome::{RepackOutcome, RepackedPack};
use crate::repack::repack_refusal::RepackRefusal;
use crate::repack::repack_request::RepackRequest;
use crate::repack::staged::{self, Cursor, Staged};
use crate::spooled_container::{commit_spooled, SpooledContainer};
use crate::sync::{complete_after_catch_up, PendingWork};
use crate::upload;

/// Rewrites the Packs holding the Entries under one folder into Packs cut in
/// Entry Path order, in one Journal batch (spec: PK-8, PK-23).
///
/// The steps, and the rule each answers to, are listed in the
/// [module documentation](crate::repack).
///
/// All or nothing. A Pack to be rewritten whose key is lost, or that was read
/// and did not verify, is [`RepackError::Refused`]: nothing is committed, the
/// new Packs are never written, and every Pack stays as it was. A run whose
/// selected Packs are all already in place commits nothing and says so with an
/// empty outcome (spec: CP-1).
///
/// After the commit the replaced Packs go to the provider's trash, and one that
/// would not go leaves the commit standing and is reported in
/// [`CommitOutcome::untrashed`](crate::commit::CommitOutcome::untrashed)
/// (spec: CP-1, OC-6). What an interrupted run leaves is the sync's to settle:
/// every staged copy and every new Pack has a pending row from before its first
/// byte, saying it was not built out of this device's files (spec: OC-2,
/// OC-7, EP-10).
pub async fn repack_folder(request: RepackRequest<'_>) -> RepackResult<RepackOutcome> {
    let RepackRequest {
        store,
        index,
        keys,
        spool,
        spool_dir,
        folder,
        target,
        batch,
        now,
        progress,
        policy,
    } = request;

    let _pending_owner = index.own_pending_rows().await?;

    progress.step(Step::begun(Phase::CatchingUp));
    let caught = catch_up(store, index, keys.control(), &policy.retry).await?;
    // A batch of this device's that the catch-up found committed is completed
    // under the ownership this run holds, before the plan reads the catalog
    // (spec: OC-2, OC-7).
    complete_after_catch_up(index, spool, now, PendingWork::Held).await;
    // What the plan reads the catalog at, and so what the batch is prepared
    // against (spec: CP-4, EP-7).
    let base = caught.head_generation();

    let plan = plan::plan(index, folder.as_ref(), target).await?;
    let replaced: Vec<usize> = (0..plan.packs.len())
        .filter(|pack| !plan.keeps(*pack))
        .collect();
    let selected: Vec<ContainerId> = plan.packs.iter().map(|held| held.summary.id).collect();
    let kept: Vec<ContainerId> = (0..plan.packs.len())
        .filter(|pack| plan.keeps(*pack))
        .map(|pack| plan.packs[pack].summary.id)
        .collect();
    if replaced.is_empty() {
        // Nothing selected, or everything selected already the cut it would
        // make: no Keyring to read and nothing to commit (spec: PK-23, CP-1).
        let outcome = RepackOutcome {
            selected,
            kept,
            removed: Vec::new(),
            added: Vec::new(),
            commit: None,
            short: None,
        };
        finished(&outcome);
        return Ok(outcome);
    }

    // A Library with a Pack in it has committed, so it has a Keyring.
    let checkpoint = index
        .checkpoint()
        .await?
        .expect("a catalog holding a current Pack stands at a committed head");
    let read = read_committed(
        store,
        keys.control(),
        &policy.retry,
        &caught.listing,
        checkpoint.keyring(),
    )
    .await?;
    // Said as the run ends, from wherever it ends, unless the commit's
    // examination of the same set speaks for it (spec: KL-15).
    let short = ShortReport::armed(read.short);
    // Every Pack to be rewritten is looked up before any is read, so a
    // key-lost one refuses the run before a byte has moved (spec: KL-17).
    let envelopes = replaced
        .iter()
        .map(|pack| envelope(&read.key_table, plan.packs[*pack].summary.id))
        .collect::<RepackResult<Vec<KeyEnvelope>>>()?;

    let new: Vec<&Segment<Member>> = plan
        .cuts
        .iter()
        .filter_map(|cut| match cut {
            Cut::New(segment) => Some(segment),
            Cut::Kept(_) => None,
        })
        .collect();
    let steps = replaced.len() + new.len();
    progress.step(Step::new(Phase::Packing, 0, steps));
    spool.prepare_dir(&spool_dir).await?;

    let mut staged: Vec<Staged> = Vec::with_capacity(replaced.len());
    let written = write(
        &Writing {
            request: Rebuilding {
                store,
                index,
                keys,
                spool,
                spool_dir: &spool_dir,
                retry: &policy.retry,
                listing: &caught.listing,
                // The copies are no part of the batch the run commits: they are
                // never uploaded, and a batch of their own keeps the run's
                // bookkeeping of what it committed about the new Packs alone.
                batch: &BatchId::new(format!("{batch}-staged")),
                now,
            },
            batch: &batch,
            plan: &plan,
            replaced: &replaced,
            envelopes: &envelopes,
            progress: |done| progress.step(Step::new(Phase::Packing, done, steps)),
        },
        &mut staged,
        &new,
    )
    .await;
    // Whatever became of the writing, the copies were only its scratch.
    staged::dispose(index, spool, &staged).await;
    let mut spooled = written?;

    upload::upload(
        store,
        index,
        spool,
        &policy.retry,
        &batch,
        progress,
        &mut spooled,
    )
    .await?;
    let commit = commit_spooled(
        store,
        index,
        keys.control(),
        &policy,
        now,
        base,
        &spooled,
        &[],
        Vec::new(),
        Vec::new(),
        Some(&short),
        progress,
    )
    .await?;
    if commit.is_some() {
        // The commit's refresh has already dropped their pending rows
        // (spec: OC-2), so the ciphertext on this device is the last thing left
        // of the batch.
        for container in &spooled {
            if let Err(error) = spool.discard(&container.spool_path).await {
                warn!(
                    container = %container.container_id,
                    reason = %error.redacted(),
                    "a committed Pack's spool file could not be removed",
                );
            }
        }
    }

    let outcome = RepackOutcome {
        selected,
        kept,
        removed: replaced
            .iter()
            .map(|pack| plan.packs[*pack].summary.id)
            .collect(),
        added: new
            .iter()
            .zip(&spooled)
            .map(|(segment, container)| RepackedPack {
                container_id: container.container_id,
                entries: container.entries.len(),
                footprint: segment.footprint.bytes(),
                written: container.ciphertext_len.get(),
            })
            .collect(),
        commit,
        short: short.unspoken(),
    };
    finished(&outcome);
    Ok(outcome)
}

/// What staging and writing are done against.
struct Writing<'a, F> {
    /// The ports, keys and clock the copies are rebuilt under, with the
    /// copies' own batch.
    request: Rebuilding<'a>,
    /// The batch the new Packs go into.
    batch: &'a BatchId,
    plan: &'a Plan,
    /// The selected Packs to rewrite, by their position in the plan.
    replaced: &'a [usize],
    /// The envelope opening each of those, in the same order.
    envelopes: &'a [KeyEnvelope],
    /// Says how many Packs have been staged and written so far.
    progress: F,
}

/// Stages every Pack to be rewritten, then writes every new Pack from the
/// copies (spec: PK-10, PK-23).
///
/// The copies go into `staged` as they are made, so the caller disposes of
/// every one of them whichever way this ends. A Pack that does not verify ends
/// it with a refusal before any new Pack is written: the run is all or
/// nothing.
async fn write<F: Fn(usize)>(
    with: &Writing<'_, F>,
    staged: &mut Vec<Staged>,
    new: &[&Segment<Member>],
) -> RepackResult<Vec<SpooledContainer>> {
    let Rebuilding {
        index, keys, spool, ..
    } = with.request;

    for (done, (pack, envelope)) in with.replaced.iter().zip(with.envelopes).enumerate() {
        let held = &with.plan.packs[*pack];
        // Every row kept and in its own order: the copy is the old Pack again,
        // verified, under a key this run holds (spec: PK-10).
        let keep = vec![true; held.table.len()];
        let replacing = Replacing {
            old: &held.summary,
            envelope,
            table: &held.table,
            deposit_times: &held.deposit_times,
            keep: &keep,
        };
        match rebuild(&with.request, &replacing).await? {
            Ok(copy) => staged.push(Staged::new(copy, keys)?),
            Err(unverified) => {
                return Err(RepackRefusal {
                    container_id: held.summary.id,
                    reason: PackRefusal::Unverified(unverified),
                }
                .into());
            }
        }
        (with.progress)(done + 1);
    }

    // Where each selected Pack's copy is, by the Pack's position in the plan.
    let copy_of: BTreeMap<usize, usize> = with
        .replaced
        .iter()
        .enumerate()
        .map(|(copy, pack)| (*pack, copy))
        .collect();
    let mut cursors: Vec<Cursor> = staged.iter().map(Cursor::new).collect();
    // Each replaced Pack is named as replaced by the first new Pack carrying any
    // of its Entries, and by that one only: the batch removes it once
    // (spec: CP-14).
    let mut named: BTreeSet<usize> = BTreeSet::new();
    let mut spooled = Vec::with_capacity(new.len());
    for (done, segment) in new.iter().enumerate() {
        let plans: Vec<EntryPlan> = segment
            .members
            .iter()
            .map(|member| member.plan.clone())
            .collect();
        // Off Storage, not off this device's disk (spec: OC-7, EP-10).
        let mut pack = PackSpool::begin(
            index,
            spool,
            with.request.spool_dir,
            with.batch,
            with.request.now,
            &plans,
            false,
        )
        .await?;
        for member in &segment.members {
            let copy = *copy_of
                .get(&member.pack)
                .expect("a new Pack carries only Entries of Packs being rewritten");
            let extent = staged[copy].copy.entries[member.row].extent.range();
            cursors[copy].carry(spool, extent, &mut pack).await?;
        }
        let written = pack.finish(index, keys).await?;
        spooled.push(carried(with, segment, written, &mut named)?);
        (with.progress)(with.replaced.len() + done + 1);
    }
    Ok(spooled)
}

/// Describes one new Pack as the commit takes it: every Entry superseding the
/// one it carries and keeping that one's deposit time (spec: PK-10, FM-15,
/// EP-7).
fn carried<F>(
    with: &Writing<'_, F>,
    segment: &Segment<Member>,
    written: crate::pack_spool::SpooledPack,
    named: &mut BTreeSet<usize>,
) -> RepackResult<SpooledContainer> {
    let numbered = |position: usize| EntryNumber::at_position(position);
    let supersedes = segment
        .members
        .iter()
        .enumerate()
        .map(|(position, member)| {
            let old = with.plan.packs[member.pack].summary.id;
            Ok((
                numbered(position)?,
                EntryRef::new(old, numbered(member.row)?),
            ))
        })
        .collect::<coffret_model::Result<BTreeMap<_, _>>>()
        .map_err(coffret_format::Error::from)?;
    // A repack puts no content into the Library: every Entry keeps the time
    // its content was deposited at (spec: PK-10, FM-15).
    let carries = segment
        .members
        .iter()
        .enumerate()
        .map(|(position, member)| {
            let held: &Held = &with.plan.packs[member.pack];
            Ok((numbered(position)?, held.deposit_times[member.row]))
        })
        .collect::<coffret_model::Result<BTreeMap<_, _>>>()
        .map_err(coffret_format::Error::from)?;
    let replaces = segment
        .members
        .iter()
        .filter(|member| named.insert(member.pack))
        .map(|member| with.plan.packs[member.pack].summary.id)
        .collect();

    let digests = written.digests;
    debug!(
        container = %written.container_id,
        entries = written.entries.len(),
        footprint = segment.footprint.bytes(),
        bytes = digests.len,
        "repacked Entries into a new Pack and spooled it",
    );
    Ok(SpooledContainer {
        container_id: written.container_id,
        kind: ContainerKind::Pack,
        spool_path: written.spool_path,
        entries: written.entries,
        envelope: written.envelope,
        ciphertext_hash: digests.blake3,
        ciphertext_len: CiphertextLenClaim::new(digests.len)
            .map_err(coffret_format::Error::from)?,
        provider_digest: digests.md5,
        object_ref: None,
        announced_at: with.request.now,
        replaces,
        supersedes,
        carries,
        materializes: false,
    })
}

/// The envelope the committed Keyring maps a Pack to rewrite to, or the
/// refusal a key-lost one is (spec: KL-7, KL-17).
///
/// Where the key table says nothing about the Pack at all, the Keyring and
/// the catalog disagree about what is current (spec: KL-7).
fn envelope(key_table: &KeyTable, container_id: ContainerId) -> RepackResult<KeyEnvelope> {
    match key_table
        .elements()
        .iter()
        .find(|element| element.container_id == container_id)
        .map(|element| element.key)
    {
        Some(ContainerKeyStatus::Envelope(envelope)) => Ok(envelope),
        Some(ContainerKeyStatus::KeyLost) => {
            warn!(
                container = %container_id,
                "a Pack the repack has to rewrite has a lost key; repacking nothing",
            );
            Err(RepackRefusal {
                container_id,
                reason: PackRefusal::KeyLost,
            }
            .into())
        }
        None => Err(RepackError::UnmappedContainer { container_id }),
    }
}

/// Records what the run came to, in counts alone: a folder is an Entry Path,
/// which may not reach a diagnostic event.
fn finished(outcome: &RepackOutcome) {
    info!(
        operation = "repack",
        selected = outcome.selected.len(),
        kept = outcome.kept.len(),
        removed = outcome.removed.len(),
        added = outcome.added.len(),
        uploaded = outcome.uploaded(),
        untrashed = outcome
            .commit
            .as_ref()
            .map_or(0, |commit| commit.untrashed.len()),
        keyring_short = outcome.short.is_some(),
        "a repack finished",
    );
}
