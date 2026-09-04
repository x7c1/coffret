use coffret_device::EntryPath;

/// Where a part named relative to `folder` stands in the Library.
///
/// Both halves are already Entry Paths — the folder came through the same
/// reading and the relative path did too — so the join has nothing left to
/// compose and nothing left to refuse (spec: EP-1, EP-2).
pub(super) fn under(folder: Option<&EntryPath>, relative: &EntryPath) -> EntryPath {
    match folder {
        None => relative.clone(),
        Some(folder) => folder.below(relative),
    }
}
