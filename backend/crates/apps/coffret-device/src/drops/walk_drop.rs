use std::path::Path;

use coffret_local_fs::UnixFs;
use coffret_usecase::drop_walk::{walk_dropped, DropWalk, DroppedItem, NotTaken, NotTakenReason};
use tracing::info;

/// Walks the paths a drop let go of: every regular file under a dropped
/// folder, depth first, and each dropped file itself (spec: EP-8).
///
/// Each path is the whole path of one item as the operating system knows it.
/// One that is empty, or names no item in a folder, is reported rather than
/// left out; so is one that is not a whole path, which nothing walks from.
/// Symbolic links are neither followed nor taken, names coffret keeps for
/// itself inside a mapped folder are reported as refused (spec: EP-11,
/// EP-14), and whatever could not be read is reported too: nothing that was
/// let go of disappears without a word.
///
/// The event it records is counts and nothing else: no path, and no name of
/// anything that was dropped (spec: EL-1).
pub async fn walk_drop(paths: &[String]) -> DropWalk {
    let mut refused = Vec::new();
    let mut items = Vec::with_capacity(paths.len());
    for path in paths {
        if path.is_empty() {
            refused.push(NotTaken {
                path: String::new(),
                reason: NotTakenReason::Empty,
            });
            continue;
        }
        let path = Path::new(path);
        let label = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !path.is_absolute() {
            refused.push(NotTaken {
                path: label,
                reason: NotTakenReason::Unreadable,
            });
            continue;
        }
        match DroppedItem::at(path) {
            Some(item) => items.push(item),
            None => refused.push(NotTaken {
                path: label,
                reason: NotTakenReason::Empty,
            }),
        }
    }

    let mut walk = walk_dropped(&UnixFs::new(), &items).await;
    refused.append(&mut walk.not_taken);
    walk.not_taken = refused;
    info!(
        operation = "walk_drop",
        items = paths.len(),
        taken = walk.taken.len(),
        not_taken = walk.not_taken.len(),
        "walked what a drop let go of",
    );
    walk
}
