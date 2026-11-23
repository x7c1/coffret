use std::collections::BTreeSet;
use std::ops::Bound;

use coffret_model::{EntryLocation, EntryPath};

use crate::commit::PendingRename;
use crate::entry_ref::EntryRef;
use crate::index::Index;
use crate::index_error::IndexResult;
use crate::rename::rename_error::RenameResult;
use crate::rename::rename_pair::RenamePair;
use crate::rename::rename_refusal::RenameRefusal;
use crate::rename::renamed_path::RenamedPath;
use crate::rename::rules::Rules;

/// What a rename will commit, worked out from the catalog alone.
pub(super) struct Plan {
    /// One per Entry renamed, as the batch carries them.
    pub(super) renames: Vec<PendingRename>,
    /// The same, as paths, ordered by the path each leaves.
    pub(super) renamed: Vec<RenamedPath>,
    /// Whether a source named Entries under it rather than one Entry alone.
    pub(super) folder: bool,
}

/// Refuses what is refused whatever the Library holds: the same path on both
/// sides, and either side in the namespace reserved for derived Entries
/// (spec: FM-15).
pub(super) fn refuse_outright(from: &EntryPath, to: &EntryPath) -> Result<(), RenameRefusal> {
    if from == to {
        return Err(RenameRefusal::SamePath);
    }
    for side in [from, to] {
        if side.is_derived() {
            return Err(RenameRefusal::ReservedNamespace { path: side.clone() });
        }
    }
    Ok(())
}

/// The pairs one request names, as the plan takes them: each refused outright
/// where it would be alone, the set refused where two of them overlap, and a
/// pair whose source is under another pair's source taken out, since moving
/// the folder moves it (spec: FM-15, EP-18).
///
/// Refused whatever the Library holds, so before anything is read: no pairs at
/// all, one source named twice with two destinations, and two destinations
/// one of which is or is under the other — the state the set would leave
/// would name one path twice, or make it a file and a folder at once. The
/// order of what is left is the order the pairs were named in.
pub(super) fn pairs_of(pairs: &[RenamePair]) -> Result<Vec<RenamePair>, RenameRefusal> {
    if pairs.is_empty() {
        return Err(RenameRefusal::Nothing);
    }
    let mut named: Vec<RenamePair> = Vec::with_capacity(pairs.len());
    for pair in pairs {
        refuse_outright(&pair.from, &pair.to)?;
        if !named.contains(pair) {
            named.push(pair.clone());
        }
    }
    let kept: Vec<RenamePair> = named
        .iter()
        .filter(|pair| {
            !named
                .iter()
                .any(|outer| outer.from != pair.from && pair.from.is_under(&outer.from))
        })
        .cloned()
        .collect();
    for (position, one) in kept.iter().enumerate() {
        for other in &kept[position + 1..] {
            let overlapping = |one: &RenamePair, other: &RenamePair| RenameRefusal::Overlapping {
                one: one.clone(),
                other: other.clone(),
            };
            if one.from == other.from || one.to == other.to || one.to.is_under(&other.to) {
                return Err(overlapping(one, other));
            }
            if other.to.is_under(&one.to) {
                return Err(overlapping(other, one));
            }
        }
    }
    Ok(kept)
}

/// Works out which Entries renaming each pair's `from` to its `to` renames, and
/// refuses a set the catalog does not admit (spec: EP-5, FM-15).
///
/// Each `from` names the Entry standing at it, if any, and every current Entry
/// under it — the subtree [`EntryPath::is_under`] covers — and each is renamed
/// by its own element, the rest of its path kept under that pair's `to`. The
/// elements are enumerated here, against the catalog as it stands, and are all
/// the record ever names: an Entry another device adds under a `from` before
/// the commit is not swept in (spec: EP-7). A pair whose `from` names nothing
/// refuses the whole set.
///
/// The state the set leaves is held to three questions about every
/// destination, against the Entries it leaves where they are: none of them
/// stands at it (spec: EP-5); none stands at a folder above it, which would
/// then be a file and a folder at once; and none stands under it, for the same
/// reason the other way round. Then to every rule FM-15's rename-set check
/// holds a record to, so that a record this plan produces is one every catalog
/// applies (spec: RV-5).
pub(super) async fn plan(index: &dyn Index, pairs: &[RenamePair]) -> RenameResult<Plan> {
    let mut renaming: BTreeSet<EntryPath> = BTreeSet::new();
    let mut renamed: Vec<(EntryRef, RenamedPath)> = Vec::new();
    let mut folder = false;
    for pair in pairs {
        let (from, to) = (&pair.from, &pair.to);
        let under: Vec<EntryLocation> = index.entries_under(Some(from)).await?;
        if under.is_empty() {
            return Err(RenameRefusal::NothingAt { path: from.clone() }.into());
        }
        let names_folder = under.iter().any(|location| location.path() != from);
        if names_folder && to.is_under(from) {
            return Err(RenameRefusal::IntoItself.into());
        }
        folder |= names_folder;
        for location in &under {
            renaming.insert(location.path().clone());
            let path = location
                .path()
                .renamed_from(from, to)
                .expect("an Entry under the source is renamed with it");
            renamed.push((
                EntryRef::of(location),
                RenamedPath {
                    from: location.path().clone(),
                    to: path,
                },
            ));
        }
    }
    renamed.sort_by(|left, right| left.1.from.cmp(&right.1.from));

    // Every destination is a pair's `to` or lies under it, so the Entries
    // standing at, above or under the pairs' destinations are every one a
    // destination can clash with.
    let destinations: Vec<&EntryPath> = pairs.iter().map(|pair| &pair.to).collect();
    let near = standing_near(index, &destinations).await?;
    let staying: BTreeSet<&str> = near
        .iter()
        .filter(|path| !renaming.contains(*path))
        .map(EntryPath::as_str)
        .collect();
    for (_, one) in &renamed {
        if let Some(refusal) = clash(&staying, &one.to) {
            return Err(refusal.into());
        }
    }

    check_rules(index, &renamed).await?;

    Ok(Plan {
        renames: renamed
            .iter()
            .map(|(entry, one)| PendingRename {
                entry: *entry,
                from: one.from.clone(),
                path: one.to.clone(),
            })
            .collect(),
        renamed: renamed.into_iter().map(|(_, one)| one).collect(),
        folder,
    })
}

/// The paths of the Entries standing at each of `destinations`, at a folder
/// above one, or under one: every Entry [`clash`] asks after for a destination
/// at or under any of them, looked up rather than the Library read.
pub(super) async fn standing_near(
    index: &dyn Index,
    destinations: &[&EntryPath],
) -> IndexResult<BTreeSet<EntryPath>> {
    let mut near: BTreeSet<EntryPath> = BTreeSet::new();
    let mut asked_above: BTreeSet<EntryPath> = BTreeSet::new();
    for &to in destinations {
        near.extend(
            index
                .entries_under(Some(to))
                .await?
                .into_iter()
                .map(|location| location.entry.path),
        );
        let mut above = to.parent();
        while let Some(folder) = above {
            above = folder.parent();
            if !asked_above.insert(folder.clone()) {
                continue;
            }
            if let Some(location) = index.entry_at(&folder).await? {
                near.insert(location.entry.path);
            }
        }
    }
    Ok(near)
}

/// Whether `to` would clash with an Entry the rename leaves where it is: stand
/// at the same path, stand under a path such an Entry stands at, or have such
/// an Entry under it.
///
/// `staying` needs to hold only the Entries [`standing_near`] finds for `to`,
/// or for a destination `to` lies at or under: no other is asked after.
pub(super) fn clash(staying: &BTreeSet<&str>, to: &EntryPath) -> Option<RenameRefusal> {
    if staying.contains(to.as_str()) {
        return Some(RenameRefusal::DestinationTaken { path: to.clone() });
    }
    let mut above = to.parent();
    while let Some(folder) = above {
        if staying.contains(folder.as_str()) {
            return Some(RenameRefusal::FileAndFolder { path: folder });
        }
        above = folder.parent();
    }
    // Every path under `to/` sorts from `to/` onwards and before anything that
    // does not start with it, so the first one at or after it is the one to ask.
    let below = format!("{to}/");
    let first = staying
        .range::<str, _>((Bound::Included(below.as_str()), Bound::Unbounded))
        .next();
    if first.is_some_and(|path| path.starts_with(&below)) {
        return Some(RenameRefusal::FileAndFolder { path: to.clone() });
    }
    None
}

/// Holds the renames to FM-15's rename-set check against the catalog as it
/// stands (spec: FM-15, RV-5).
async fn check_rules(index: &dyn Index, renamed: &[(EntryRef, RenamedPath)]) -> RenameResult<()> {
    match Rules::read(index, renamed).await?.refusal(renamed)? {
        Some(refusal) => Err(refusal.into()),
        None => Ok(()),
    }
}
