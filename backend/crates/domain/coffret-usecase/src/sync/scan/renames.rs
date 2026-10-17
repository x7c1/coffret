use std::collections::{BTreeMap, BTreeSet};

use coffret_model::ContentHash;
use tracing::debug;

use crate::commit::PendingRename;
use crate::device_state::LocalEntry;
use crate::entry_ref::EntryRef;
use crate::index::Index;
use crate::mapped_roots::MappedRoots;
use crate::rename::refuse_detected;
use crate::sync::scan::deletions::{Deletions, Gone};
use crate::sync::surfaced::Surfaced;
use crate::sync::survey::Survey;
use crate::sync::sync_error::SyncResult;

/// What a file holds, as far as telling one file's content from another's
/// takes: its length and its content hash (spec: FM-9).
type Content = (u64, ContentHash);

/// Pairs the files this device materialized that are gone with the files that
/// appeared in their place, where the pair can only be one file renamed, and
/// reports every other gone file as deleted locally (spec: EP-10).
///
/// A gone file and an appeared one pair only where all of these hold:
///
/// - the gone file's record is this device's copy of the Entry standing at its
///   path, and not one waiting to be relocated (the scan reads no such record
///   as gone at all);
/// - the appeared file is new to the scan — no Entry at its path and no record
///   of this device's there — and stands under the same mapped root;
/// - its length and content hash are the gone file's Entry's;
/// - and they are the only two files the scan knows to hold that content: one
///   gone, one appeared, and none of this device's files still standing with
///   it. Two gone, two appeared, or a copy of it still on disk beside them, and
///   which file became which is a guess, so nothing pairs and every one of them
///   is what it would have been without this step.
///
/// An appeared file is hashed only where its length is a gone file's Entry's,
/// so a scan with no gone file reads nothing more than it did.
///
/// A pair is then held to every rule a rename planned with
/// [`rename_entries`](crate::rename::rename_entries) is held to. One that
/// passes leaves the scan's candidates and its deletions, and becomes a rename
/// in [`Survey::renames`]: no upload, and the Entry gets the appeared file's
/// path in the batch the sync commits. One that is refused stays what it would
/// have been — the appeared file new, the gone one deleted locally — and is
/// reported with the refusal, so a rename this device could have carried and
/// did not is never silent.
///
/// A gone file that pairs with nothing keeps its record and is reported
/// deleted locally, so a later scan whose scope covers its partner can still
/// pair them.
pub(super) async fn pair_renames(
    index: &dyn Index,
    roots: &dyn MappedRoots,
    deletions: Deletions,
    survey: &mut Survey,
) -> SyncResult<()> {
    let Deletions { gone, kept } = deletions;

    // What each gone file's Entry holds, for the ones that are this device's
    // copies of the Entry at their path.
    let mut held: BTreeMap<Content, Vec<usize>> = BTreeMap::new();
    for (position, one) in gone.iter().enumerate() {
        if one.local.is_copy_of(&one.location) {
            held.entry(content_of(one)).or_default().push(position);
        }
    }
    let lengths: BTreeSet<u64> = held.keys().map(|(size, _)| *size).collect();

    // The files that appeared, hashed only where a gone file's length says
    // they could be one of them.
    let mut appeared: BTreeMap<Content, Vec<usize>> = BTreeMap::new();
    if !lengths.is_empty() {
        for (position, candidate) in survey.candidates.iter().enumerate() {
            if candidate.replaces.is_some() || !lengths.contains(&candidate.source.size) {
                continue;
            }
            if index
                .local_entry_at(&candidate.source.path)
                .await?
                .is_some()
            {
                continue;
            }
            let hash = candidate.source.content_hash(roots).await?;
            appeared
                .entry((candidate.source.size, hash))
                .or_default()
                .push(position);
        }
    }

    // A copy this device still has of the same content makes the pair as
    // ambiguous as a second gone file would.
    let standing = standing_contents(&kept);

    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for (content, gone_at) in &held {
        let ([gone_at], Some([appeared_at])) =
            (gone_at.as_slice(), appeared.get(content).map(Vec::as_slice))
        else {
            continue;
        };
        if standing.contains(content) {
            continue;
        }
        if gone[*gone_at].root != survey.candidates[*appeared_at].source.root {
            continue;
        }
        pairs.push((*gone_at, *appeared_at));
    }

    let detected: Vec<PendingRename> = pairs
        .iter()
        .map(|(gone_at, appeared_at)| PendingRename {
            entry: EntryRef::of(&gone[*gone_at].location),
            from: gone[*gone_at].location.path().clone(),
            path: survey.candidates[*appeared_at].source.path.clone(),
        })
        .collect();
    let refusals = if detected.is_empty() {
        Vec::new()
    } else {
        refuse_detected(index, &detected).await?
    };

    let mut paired_gone = BTreeSet::new();
    let mut paired_appeared = BTreeSet::new();
    let mut refused = 0usize;
    for ((rename, refusal), (gone_at, appeared_at)) in detected.into_iter().zip(refusals).zip(pairs)
    {
        match refusal {
            None => {
                paired_gone.insert(gone_at);
                paired_appeared.insert(appeared_at);
                survey.renames.push(rename);
            }
            Some(refusal) => {
                refused += 1;
                survey.surfaced.push(Surfaced::RenameRefused {
                    path: rename.path,
                    from: rename.from,
                    refusal,
                });
            }
        }
    }

    survey
        .renames
        .sort_by(|left, right| left.from.cmp(&right.from));

    if !paired_appeared.is_empty() {
        let mut position = 0;
        survey.candidates.retain(|_| {
            let keep = !paired_appeared.contains(&position);
            position += 1;
            keep
        });
    }
    for (position, one) in gone.into_iter().enumerate() {
        if !paired_gone.contains(&position) {
            survey.surfaced.push(Surfaced::DeletedLocally {
                path: one.location.path().clone(),
            });
        }
    }

    // Counts only: an Entry Path may not reach a diagnostic event.
    debug!(
        renamed = survey.renames.len(),
        refused, "paired gone files with the files that appeared in their place",
    );
    Ok(())
}

/// What a gone file's Entry holds.
fn content_of(gone: &Gone) -> Content {
    (gone.location.entry.extent.size(), gone.location.entry.hash)
}

/// What the files this device still has hold, as its records last saw them.
///
/// A record that carries no hash says nothing about its content and is left
/// out: the scan read nothing to learn it, and asking the disk for it would
/// read every file this device holds.
fn standing_contents(kept: &[LocalEntry]) -> BTreeSet<Content> {
    kept.iter()
        .filter_map(|local| {
            local
                .observation
                .hash
                .map(|hash| (local.observation.size, hash))
        })
        .collect()
}
