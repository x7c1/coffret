use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use coffret_model::EntryPath;
use tracing::debug;

use crate::device_state::Mapping;
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::local_place::LocalPlace;
use crate::fetch::target::Target;
use crate::index::Index;

/// Where on this device the file for one Entry belongs (spec: EP-9).
///
/// EP-9 is one rule, so it has one implementation, and this is the door onto it
/// for callers outside the fetch. A reader that wants the bytes of an Entry this
/// device already has needs the very translation a fetch performs before it
/// places one, and deriving it a second time elsewhere would be two readings of
/// the mappings with nothing keeping them in agreement — which is what EP-4's
/// posture forbids the answer to a path from depending on.
///
/// The path is where the file *belongs*, and never a claim that it is there:
/// only a present materialization record says that (spec: EP-10). A caller
/// holding no such record asks a fetch instead.
///
/// The catalog is read as it stands. Unlike [`fetch_entry`](super::fetch_entry)
/// this catches nothing up first: a catch-up is a read of Storage, and a caller
/// that has just fetched — or that is answering out of its own materialization
/// record — has no question the Library's head would settle.
///
/// # Errors
///
/// [`FetchError::EntryNotCurrent`] where the Library holds no current Entry at
/// the path, [`FetchError::UnmappedEntryPath`] where it holds one no mapping of
/// this device reaches, and [`FetchError::UnmaterializablePath`] where a mapping
/// does reach it and no file here can stand for it — an Entry standing at
/// exactly a mapping's own prefix, or a component no local name may be made of
/// (spec: EP-2, EP-4). The three are separate because they are different
/// verdicts: the first is about the Library, the other two about this device.
///
/// A catalog that could not be read is none of the three and travels as
/// [`FetchError::Index`], having decided nothing about the path.
pub async fn local_path_of(index: &dyn Index, path: &EntryPath) -> FetchResult<PathBuf> {
    Ok(target_of(index, path).await?.place.to_path_buf())
}

/// Where on this device a file standing at `path` would go, whether or not the
/// Library holds an Entry there (spec: EP-9).
///
/// [`local_path_of`] asks the same question of an Entry the catalog holds, and
/// this asks it of a path — which is the question something *adding* a file has:
/// nothing stands there yet, and where it goes is settled by the mappings alone
/// rather than by a row. Both come out of `translate`, because EP-9 has one
/// implementation and a second reading of the mappings is what would let a file
/// be written somewhere a fetch would never look for it.
///
/// It says nothing about what is on disk. A path a file already stands at and
/// one nothing stands at answer alike, because the mappings answer alike about
/// them; whoever writes there is who decides what to do about a file already
/// there.
///
/// # Errors
///
/// [`FetchError::UnmappedEntryPath`] where no mapping of this device reaches the
/// path, and [`FetchError::UnmaterializablePath`] where one does and no file
/// here can stand for it — a path that is exactly a mapping's own prefix, or a
/// component no local name may be made of (spec: EP-2, EP-4). There is no
/// `EntryNotCurrent` among them, and that is the whole difference: the Library
/// holding nothing at the path is the ordinary case here rather than a refusal.
///
/// A catalog whose mappings could not be read is neither of the two and travels
/// as [`FetchError::Index`], having decided nothing about the path.
pub async fn local_path_for(index: &dyn Index, path: &EntryPath) -> FetchResult<PathBuf> {
    Ok(local_place_for(index, path).await?.to_path_buf())
}

/// The same answer in the form something *writing* there needs (spec: EP-9).
///
/// [`local_path_for`] joins the mapped root and the components below the
/// mapping's prefix into one path, which is everything a reader wants and not
/// enough for a writer: handing that string to the operating system is what lets
/// a symbolic link on the way down be followed out of the mapped folder. A
/// [`LocalPlace`] keeps the two halves apart so that
/// [`descend`](LocalPlace::descend) can walk them one component at a time
/// (spec: EP-4, EP-11).
///
/// # Errors
///
/// Exactly [`local_path_for`]'s, and for the same reasons — nothing here touches
/// the filesystem, so a place that cannot be descended into is still a place.
pub async fn local_place_for(index: &dyn Index, path: &EntryPath) -> FetchResult<LocalPlace> {
    let mappings = index.mappings().await?;
    let mapping = reaching(&mappings, path)
        .ok_or_else(|| FetchError::UnmappedEntryPath { path: path.clone() })?;
    translate(&mapping.local_root, mapping.prefix.as_ref(), path)
}

/// The folder on this device that stands for one folder of the Library, or
/// `None` where no mapping reaches it; `None` in is the Library root.
///
/// A folder is not an Entry Path standing for a file, so it cannot go through
/// `translate` unconditionally: a folder that is exactly a mapping's prefix is
/// the mapped root itself, which is the one case that rule refuses — rightly, of
/// a file. Everything below it is the same translation as any other path.
///
/// `None` rather than a refusal, because a folder no mapping reaches is an
/// ordinary answer about a Library a device holds part of (spec: EP-9): whoever
/// asked is reading a folder that is not on this device, which is a thing to say
/// rather than a failure to report.
///
/// # Errors
///
/// [`FetchError::UnmaterializablePath`] alone, where a mapping does reach the
/// folder and no local name can be made of a component below the prefix
/// (spec: EP-2, EP-4). `UnmappedEntryPath` is not among them by construction —
/// that verdict is the `None` above — and neither is `EntryNotCurrent`, there
/// being no Entry in the question at all.
///
/// A catalog whose mappings could not be read is not that either and travels as
/// [`FetchError::Index`], having decided nothing about the folder: it is not to
/// be read as the `None` that says the folder is elsewhere.
pub async fn local_folder_for(
    index: &dyn Index,
    folder: Option<&EntryPath>,
) -> FetchResult<Option<PathBuf>> {
    let mappings = index.mappings().await?;
    let Some(folder) = folder else {
        // The Library root is reached by a root mapping alone: a top-level
        // mapping stands for its own subtree and not for what sits beside it.
        return Ok(mappings
            .iter()
            .find(|mapping| mapping.prefix.is_none())
            .map(|mapping| mapping.local_root.clone()));
    };
    let Some(mapping) = reaching(&mappings, folder) else {
        return Ok(None);
    };
    if mapping.prefix.as_ref() == Some(folder) {
        return Ok(Some(mapping.local_root.clone()));
    }
    translate(&mapping.local_root, mapping.prefix.as_ref(), folder)
        .map(|place| Some(place.to_path_buf()))
}

/// The one mapping that stands for a path, where any does (spec: EP-9).
///
/// The mappings partition the Library's namespace: a top-level mapping
/// represents its own subtree, and a Library-root mapping represents whatever
/// the top-level ones do not. So the path's top-level component decides, and a
/// root mapping is the answer only where nothing claims that component — which
/// is the same partition [`targets`] walks from the other end.
fn reaching<'a>(mappings: &'a [Mapping], path: &EntryPath) -> Option<&'a Mapping> {
    mappings
        .iter()
        .find(|mapping| {
            mapping
                .prefix
                .as_ref()
                .is_some_and(|prefix| prefix.as_str() == path.top_level())
        })
        .or_else(|| mappings.iter().find(|mapping| mapping.prefix.is_none()))
}

/// The one Entry at `path`, with the local path a mapping gives it.
///
/// The prefix that narrows the mappings is the Entry Path itself, so the range
/// scan behind it covers that path and its subtree; everything but the Entry
/// standing at exactly `path` is then dropped (spec: EP-9).
pub(super) async fn target_of(index: &dyn Index, path: &EntryPath) -> FetchResult<Target> {
    let mut translated = targets(index, Some(path)).await?;
    translated.retain(|target| target.path() == path);
    match translated.pop() {
        Some(target) => Ok(target),
        // A path no mapping reaches and a path the Library holds nothing at look
        // identical from here and are different verdicts, so the catalog is
        // asked which of the two this is (spec: EP-5, EP-9).
        None => Err(match index.entry_at(path).await? {
            Some(_) => FetchError::UnmappedEntryPath { path: path.clone() },
            None => FetchError::EntryNotCurrent { path: path.clone() },
        }),
    }
}

/// Every Entry this run could place, at the local path a mapping gives it.
///
/// The mappings decide the whole of what a fetch may touch: a mapping is what
/// translates an Entry Path into a local path at all, so a subtree nothing maps
/// is a subtree no fetch reaches, however the request narrows (spec: EP-9). The
/// request's prefix therefore *intersects* each mapping rather than being
/// consulted instead of it.
///
/// Where both kinds of mapping are present the Library-root one stands for the
/// remainder, so a top-level component another mapping represents is left to it
/// — the mirror of what a scan does when it walks the root-mapped folder and
/// stops at those names (spec: EP-9). Between them the mappings partition the
/// namespace, which is why one Entry Path is reached through at most one of
/// them.
///
/// Two Entry Paths landing on one local path is refused rather than resolved,
/// from the placing side of the rule the scan meets from the other: a device
/// that cannot hold both files does not get to have one of them silently chosen
/// (spec: EP-4).
pub(super) async fn targets(
    index: &dyn Index,
    prefix: Option<&EntryPath>,
) -> FetchResult<Vec<Target>> {
    let mappings = index.mappings().await?;
    // Every question below holds a mapping's prefix against an Entry Path the
    // catalog answered with, and both are `EntryPath`s and so in one spelling
    // (spec: EP-1, EP-3). A prefix in any other one would stand for a subtree
    // the catalog never answers with, and a fetch would quietly place nothing
    // where the user pointed it (spec: EP-9).
    let claimed: BTreeSet<&str> = mappings
        .iter()
        .filter_map(|mapping| mapping.prefix.as_ref())
        .map(EntryPath::as_str)
        .collect();

    let mut targets: BTreeMap<EntryPath, Target> = BTreeMap::new();
    let mut locals: BTreeMap<PathBuf, EntryPath> = BTreeMap::new();

    for mapping in &mappings {
        let mapped_prefix = mapping.prefix.as_ref();
        let Some(scope) = narrow(mapped_prefix, prefix) else {
            // The request and this mapping cover disjoint subtrees.
            continue;
        };
        for location in index.entries_under(scope.as_ref()).await? {
            // A top-level mapping represents its own subtree, so the Library-root
            // mapping represents what is left (spec: EP-9).
            if mapped_prefix.is_none() && claimed.contains(location.path().top_level()) {
                continue;
            }
            let place = translate(&mapping.local_root, mapped_prefix, location.path())?;
            if let Some(held) = locals.insert(place.to_path_buf(), location.path().clone()) {
                return Err(FetchError::LocalPathCollision {
                    first: held,
                    second: location.path().clone(),
                });
            }
            targets.insert(location.path().clone(), Target { location, place });
        }
    }

    debug!(
        mappings = mappings.len(),
        narrowed = prefix.is_some(),
        targets = targets.len(),
        "the mappings translated the Library's Entries into local paths",
    );
    Ok(targets.into_values().collect())
}

/// The subtree one mapping contributes to this run, or `None` where it
/// contributes nothing.
///
/// `Some(None)` is the whole Library, which only an unnarrowed run of a
/// Library-root mapping asks for. Where a prefix and a mapping overlap the
/// deeper of the two wins, because the run covers exactly their intersection:
/// the mapping bounds where files may go and the prefix bounds what the caller
/// asked for.
fn narrow(mapping: Option<&EntryPath>, request: Option<&EntryPath>) -> Option<Option<EntryPath>> {
    let Some(request) = request else {
        return Some(mapping.cloned());
    };
    let Some(mapping) = mapping else {
        return Some(Some(request.clone()));
    };
    if request.is_under(mapping) {
        Some(Some(request.clone()))
    } else if mapping.is_under(request) {
        Some(Some(mapping.clone()))
    } else {
        None
    }
}

/// Where one Entry's file goes under one mapping (spec: EP-9).
///
/// `prefix` is the mapping's, in the same form the Entry Paths it is stripped
/// from are in (spec: EP-1); `local_root` is the folder it is rooted at, which
/// is a local path and normalizes nowhere — what the operating system was given
/// is what it is asked for again.
///
/// The components are kept apart from the root rather than joined onto it, and
/// that is the point: they are handed to the descent one at a time, so nothing
/// here ever builds a string an operating system could read as climbing out of
/// the mapped folder (spec: EP-4). That none of them is empty, `.`, `..`, or
/// carrying a NUL is the [`EntryPath`]'s own to answer (spec: EP-2), and it is
/// not asked again here: a path that could be one of those is a path this type
/// cannot hold.
///
/// Which components a path is made of is settled here and what is *on disk* at
/// them is not: a component this splitting produces may still be a symbolic
/// link on this device, and refusing that is the descent's
/// ([`LocalPlace::descend`]), because the answer is only worth having while the
/// folder is held open.
///
/// An Entry standing at exactly a mapping's own prefix is refused before the
/// split is reached: stripping the prefix leaves no separator to strip after
/// it, so there is no relative path to take components off. The local root is
/// the folder the subtree lives in, and it cannot also be a file in it.
fn translate(
    local_root: &Path,
    prefix: Option<&EntryPath>,
    path: &EntryPath,
) -> FetchResult<LocalPlace> {
    // No folder to name: nothing on disk has been reached at this point, and
    // the path itself is the whole of the verdict.
    let unmaterializable = || FetchError::UnmaterializablePath {
        path: path.clone(),
        component: None,
    };

    let relative = match prefix {
        None => path.as_str(),
        Some(prefix) => path
            .as_str()
            .strip_prefix(prefix.as_str())
            .and_then(|rest| rest.strip_prefix('/'))
            .ok_or_else(unmaterializable)?,
    };

    let components = relative.split('/').map(str::to_owned).collect();
    Ok(LocalPlace::new(local_root.to_path_buf(), components))
}
