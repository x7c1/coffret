//! Whether a path a change would put an Entry at makes one Entry Path both an
//! Entry and a folder of Entries.
//!
//! No device can materialize both, because a file system gives one name in a
//! folder to one thing (spec: EP-4). So a rename is refused where the state it
//! would leave holds such a pair (spec: EP-5), and so is an addition
//! (spec: EP-6). Both ask the same two questions of the Entries the state
//! keeps — is one standing at a folder above the new path, is one standing
//! under it — and both ask the Index only about the paths involved rather than
//! read every Entry of the Library. The questions are written once, here.

use std::borrow::Borrow;
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Bound;

use coffret_model::{ContainerId, EntryLocation, EntryPath};

use crate::index::Index;
use crate::index_error::IndexResult;

/// The current Entries standing at each of `paths`, at a folder above one, or
/// under one, by the path each stands at: every Entry [`clash`] asks after for
/// a path at or under any of them, looked up rather than the Library read.
pub(crate) async fn standing_near(
    index: &dyn Index,
    paths: &[&EntryPath],
) -> IndexResult<BTreeMap<EntryPath, EntryLocation>> {
    let mut near: BTreeMap<EntryPath, EntryLocation> = BTreeMap::new();
    let mut asked_above: BTreeSet<EntryPath> = BTreeSet::new();
    for &path in paths {
        for location in index.entries_under(Some(path)).await? {
            near.insert(location.path().clone(), location);
        }
        let mut above = path.parent();
        while let Some(folder) = above {
            above = folder.parent();
            if !asked_above.insert(folder.clone()) {
                continue;
            }
            if let Some(location) = index.entry_at(&folder).await? {
                near.insert(folder, location);
            }
        }
    }
    Ok(near)
}

/// What a new path meets among the Entries a change leaves standing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Clash {
    /// An Entry stands at the path itself (spec: EP-5).
    Taken(EntryPath),
    /// The path would be both a file and a folder: this one, at a folder above
    /// the new path an Entry stands at, or the new path itself with Entries
    /// standing under it (spec: EP-4).
    FileAndFolder(EntryPath),
}

/// Whether `to` would clash with an Entry in `staying`: stand at the same
/// path, stand under a path such an Entry stands at, or have such an Entry
/// under it.
///
/// `staying` needs to hold only the Entries [`standing_near`] finds for `to`,
/// or for a path `to` lies at or under, together with what the change itself
/// puts there: no other is asked after.
pub(crate) fn clash<S: Borrow<str> + Ord>(staying: &BTreeSet<S>, to: &EntryPath) -> Option<Clash> {
    if staying.contains(to.as_str()) {
        return Some(Clash::Taken(to.clone()));
    }
    file_and_folder(staying, to).map(Clash::FileAndFolder)
}

/// The path that would be both a file and a folder were an Entry put at `to`
/// beside the Entries in `staying`, if any: the nearest folder above `to` an
/// Entry stands at, or `to` itself where Entries stand under it.
///
/// An Entry at `to` itself is not asked about: that is a question of one path
/// held twice, which [`clash`] asks first.
pub(crate) fn file_and_folder<S: Borrow<str> + Ord>(
    staying: &BTreeSet<S>,
    to: &EntryPath,
) -> Option<EntryPath> {
    let mut above = to.parent();
    while let Some(folder) = above {
        if staying.contains(folder.as_str()) {
            return Some(folder);
        }
        above = folder.parent();
    }
    // Every path under `to/` sorts from `to/` onwards and before anything that
    // does not start with it, so the first one at or after it is the one to ask.
    let below = format!("{to}/");
    let first = staying
        .range::<str, _>((Bound::Included(below.as_str()), Bound::Unbounded))
        .next();
    first
        .is_some_and(|path| path.borrow().starts_with(&below))
        .then(|| to.clone())
}

/// One Entry a change puts at a path: an addition, and the Container the same
/// change removes from under it, if any — the one-file Container a replacement
/// supersedes or a Pack absorbs.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Adding<'a> {
    /// Where the Entry is added.
    pub(crate) path: &'a EntryPath,
    /// The Container the change removes, whose Entries leave before any
    /// addition enters (spec: EP-6).
    pub(crate) removes: Option<ContainerId>,
}

/// The Entries a change leaves standing around `paths`, once the Containers
/// it removes have left and its renames have taken effect: every Entry
/// [`file_and_folder`] asks after for an addition at one of `paths`
/// (spec: EP-6, FM-15).
///
/// `renames` are the `(from, to)` of every Entry the change renames: each
/// leaves its `from` and stands at its `to`.
pub(crate) async fn staying_near(
    index: &dyn Index,
    paths: &[&EntryPath],
    removed: &BTreeSet<ContainerId>,
    renames: &[(&EntryPath, &EntryPath)],
) -> IndexResult<BTreeSet<String>> {
    let vacated: BTreeSet<&EntryPath> = renames.iter().map(|(from, _)| *from).collect();
    let mut staying: BTreeSet<String> = standing_near(index, paths)
        .await?
        .into_iter()
        .filter(|(path, location)| {
            !removed.contains(&location.container_id) && !vacated.contains(path)
        })
        .map(|(path, _)| path.as_str().to_owned())
        .collect();
    staying.extend(renames.iter().map(|(_, to)| to.as_str().to_owned()));
    Ok(staying)
}

/// For each of `adding`, in the order given, the path its addition would make
/// both a file and a folder, or `None` where it may be added (spec: EP-6).
///
/// What a writer asks before it prepares a batch, so that a file it cannot add
/// is left out and reported rather than refusing the whole commit. The
/// Containers the additions remove leave first and `renames` take effect, as
/// they do at the commit; an addition admitted earlier in the order then
/// stands for every later one, and one refused does not.
pub(crate) async fn refuse_additions(
    index: &dyn Index,
    adding: &[Adding<'_>],
    renames: &[(&EntryPath, &EntryPath)],
) -> IndexResult<Vec<Option<EntryPath>>> {
    if adding.is_empty() {
        return Ok(Vec::new());
    }
    let paths: Vec<&EntryPath> = adding.iter().map(|one| one.path).collect();
    let removed: BTreeSet<ContainerId> = adding.iter().filter_map(|one| one.removes).collect();
    let mut staying = staying_near(index, &paths, &removed, renames).await?;
    let mut refusals = Vec::with_capacity(adding.len());
    for one in adding {
        let refusal = file_and_folder(&staying, one.path);
        if refusal.is_none() {
            staying.insert(one.path.as_str().to_owned());
        }
        refusals.push(refusal);
    }
    Ok(refusals)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{clash, file_and_folder, Clash};
    use crate::entry_paths::entry_path;

    /// A path is both a file and a folder where an Entry stands at a folder
    /// above it or under it, and the nearest folder above is the one named;
    /// a sibling whose name merely begins with it is neither (spec: EP-4).
    #[test]
    fn a_path_is_both_where_an_entry_stands_above_or_under_it() {
        let staying = BTreeSet::from(["docs", "notes/x.pdf", "notes-annex/y.pdf"]);
        assert_eq!(
            file_and_folder(&staying, &entry_path("docs/a/b.pdf")),
            Some(entry_path("docs"))
        );
        assert_eq!(
            file_and_folder(&staying, &entry_path("notes")),
            Some(entry_path("notes"))
        );
        assert_eq!(file_and_folder(&staying, &entry_path("note")), None);
        assert_eq!(
            file_and_folder(&staying, &entry_path("notes-annex")),
            Some(entry_path("notes-annex"))
        );
        assert_eq!(file_and_folder(&staying, &entry_path("docs")), None);
        assert_eq!(
            clash(&staying, &entry_path("docs")),
            Some(Clash::Taken(entry_path("docs")))
        );
    }
}
