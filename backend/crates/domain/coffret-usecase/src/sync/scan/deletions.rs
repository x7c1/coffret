use std::collections::{BTreeMap, BTreeSet};

use coffret_model::EntryPath;

use crate::index::Index;
use crate::local_scan::{RootState, SourceFile, WalkedRoot};
use crate::sync::surfaced::Surfaced;
use crate::sync::sync_error::SyncResult;

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
pub(super) async fn deletions(
    index: &dyn Index,
    roots: &[WalkedRoot],
    found: &BTreeMap<EntryPath, SourceFile>,
) -> SyncResult<Vec<Surfaced>> {
    // Every mapping's prefix, available or not: the same set, built the same way
    // and for the same reason, as the walk's (spec: EP-9, EP-12) — the walk's
    // own spelling of each prefix, not the recorded one.
    let claimed: BTreeSet<&str> = roots
        .iter()
        .filter_map(|root| root.prefix.as_ref())
        .map(EntryPath::as_str)
        .collect();

    let mut gone = BTreeSet::new();
    for root in roots {
        if matches!(root.state, RootState::Unavailable(_)) {
            continue;
        }
        let prefix = root.prefix.as_ref();
        for local in index.present_under(prefix).await? {
            // A prefixed mapping's answer is already bounded to its own subtree;
            // the root mapping's is the whole present set, so the subtrees other
            // mappings stand for come out of it here.
            if prefix.is_none() && claimed.contains(local.observation.path.top_level()) {
                continue;
            }
            if !found.contains_key(&local.observation.path) {
                gone.insert(local.observation.path);
            }
        }
    }
    Ok(gone
        .into_iter()
        .map(|path| Surfaced::DeletedLocally { path })
        .collect())
}
