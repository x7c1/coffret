use std::collections::BTreeMap;
use std::path::PathBuf;

use coffret_model::{EntryLocation, EntryPath};

use crate::device_state::{LocalEntry, Mapping};
use crate::index::Index;
use crate::local_scan::{RootState, SourceFile, WalkedRoot};
use crate::sync::survey::Survey;
use crate::sync::sync_error::SyncResult;

/// A file this device materialized that the scan found gone, at a path the
/// Library still holds an Entry at: a deletion this device witnessed, unless it
/// pairs with a file that appeared as a rename (spec: EP-10).
pub(super) struct Gone {
    /// This device's record of the file.
    pub(super) local: LocalEntry,
    /// The Entry standing at the path the file stood at.
    pub(super) location: EntryLocation,
    /// The local root of the mapping the file stood under, which is the one
    /// root a file it pairs with has to stand under too (spec: EP-9).
    pub(super) root: PathBuf,
}

/// What the scan made of this device's records under the roots it walked.
pub(super) struct Deletions {
    /// The files gone from disk whose Entries are still in the Library, in
    /// Entry Path order.
    pub(super) gone: Vec<Gone>,
    /// The records whose files the walk found where they stand.
    pub(super) kept: Vec<LocalEntry>,
}

/// The files this device materialized that are no longer on disk (spec: EP-10).
///
/// Only rows the port reports under a mapped prefix are considered, so a
/// subtree this device does not map is not a subtree of deletions. A row whose
/// path no walk found is a file the device put there and lost; one path can be
/// reported by two mappings, so the findings are deduplicated by path.
///
/// A mapping whose root the device cannot vouch for produces no deletion at all
/// (spec: EP-12). Nothing under such a root was walked, so every row beneath it
/// would be absent from `found` for a reason that has nothing to do with the
/// files: an unplugged disk must never read as the user having emptied the
/// folder.
///
/// That has to be applied as the same partition the walk applies, and not merely
/// as skipping the unavailable mapping's own pass. A top-level mapping represents
/// its subtree and the Library-root mapping represents *the remainder*
/// (spec: EP-9), while `present_under(None)` answers with every present row the
/// device holds — the rows under a top-level mapping's prefix included. So the
/// root mapping accounts for the remainder here too: a row under a prefix another
/// mapping stands for belongs to that mapping, and if that mapping is
/// unavailable the row is nobody's evidence.
///
/// A row whose Entry has left the Library is no deletion to report: the Library
/// no longer holds what the file was a copy of, and the disk no longer holds
/// the file. Whether the sync discarded it or the person moved or removed it
/// themselves, nothing is left for a finding to be about, so the row goes into
/// [`Survey::forgotten`] instead (spec: EP-15).
///
/// A gone file whose Entry is still current is handed back rather than
/// reported here: whether it is a deletion or one half of a rename is decided
/// once the files that appeared are known (spec: EP-10). The records whose
/// files are still there are handed back too, because a file holding the same
/// content as a gone one makes the pair ambiguous.
pub(super) async fn deletions(
    index: &dyn Index,
    roots: &[WalkedRoot],
    found: &BTreeMap<EntryPath, SourceFile>,
    survey: &mut Survey,
) -> SyncResult<Deletions> {
    // Every mapping's prefix, available or not: the same set the walk holds its
    // root mapping to (spec: EP-9, EP-12).
    let represented_elsewhere =
        Mapping::represented_prefixes(roots.iter().map(|root| &root.mapping));

    let mut gone: BTreeMap<EntryPath, (LocalEntry, PathBuf)> = BTreeMap::new();
    let mut kept: BTreeMap<EntryPath, LocalEntry> = BTreeMap::new();
    for root in roots {
        if matches!(root.state, RootState::Unavailable(_)) {
            continue;
        }
        let prefix = root.mapping.prefix.as_ref();
        for local in index.present_under(prefix).await? {
            // A prefixed mapping's answer is already bounded to its own subtree;
            // the root mapping's is the whole present set, so the subtrees other
            // mappings stand for come out of it here.
            if prefix.is_none()
                && represented_elsewhere.contains(local.observation.path.top_level())
            {
                continue;
            }
            let path = local.observation.path.clone();
            if found.contains_key(&path) {
                kept.entry(path).or_insert(local);
            } else {
                gone.entry(path)
                    .or_insert_with(|| (local, root.mapping.local_root.clone()));
            }
        }
    }
    let mut current = Vec::new();
    for (path, (local, root)) in gone {
        match index.entry_at(&path).await? {
            Some(location) => current.push(Gone {
                local,
                location,
                root,
            }),
            None => survey.forgotten.push(path),
        }
    }
    Ok(Deletions {
        gone: current,
        kept: kept.into_values().collect(),
    })
}
