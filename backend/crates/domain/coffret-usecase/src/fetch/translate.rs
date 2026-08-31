use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use coffret_model::EntryPath;
use tracing::debug;

use crate::fetch::fetch_error::{FetchError, FetchResult};
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
    Ok(target_of(index, path).await?.local_path)
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
            let local_path = translate(&mapping.local_root, mapped_prefix, location.path())?;
            if let Some(held) = locals.insert(local_path.clone(), location.path().clone()) {
                return Err(FetchError::LocalPathCollision {
                    first: held,
                    second: location.path().clone(),
                });
            }
            targets.insert(
                location.path().clone(),
                Target {
                    location,
                    local_path,
                },
            );
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
/// The path is rebuilt component by component rather than joined wholesale, and
/// that is the point: an Entry Path is an authenticated value but not a
/// validated one, and a component that is empty, `.`, `..`, or carries a NUL
/// would either climb out of the mapped folder or name something other than
/// what the Library holds. None of those is sanitized into a different local
/// name — coffret never invents one (spec: EP-2, EP-4).
///
/// An Entry standing at exactly a mapping's own prefix is refused before the
/// loop is reached: stripping the prefix leaves no separator to strip after it,
/// so there is no relative path to rebuild. The local root is the folder the
/// subtree lives in, and it cannot also be a file in it.
fn translate(
    local_root: &Path,
    prefix: Option<&EntryPath>,
    path: &EntryPath,
) -> FetchResult<PathBuf> {
    let unmaterializable = || FetchError::UnmaterializablePath { path: path.clone() };

    let relative = match prefix {
        None => path.as_str(),
        Some(prefix) => path
            .as_str()
            .strip_prefix(prefix.as_str())
            .and_then(|rest| rest.strip_prefix('/'))
            .ok_or_else(unmaterializable)?,
    };

    let mut local = local_root.to_path_buf();
    for component in relative.split('/') {
        if component.is_empty() || component == "." || component == ".." || component.contains('\0')
        {
            return Err(unmaterializable());
        }
        local.push(component);
    }
    Ok(local)
}
