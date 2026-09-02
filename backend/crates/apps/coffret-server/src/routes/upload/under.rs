use coffret_device::EntryPath;

/// Where a part named relative to `folder` stands in the Library.
///
/// Both halves are already the Library's spelling — the folder came through the
/// same shaping and the relative path did too — so composing them changes nothing
/// (spec: EP-1).
pub(super) fn under(folder: Option<&EntryPath>, relative: &EntryPath) -> EntryPath {
    match folder {
        None => relative.clone(),
        Some(folder) => EntryPath::nfc(format!("{}/{}", folder.as_str(), relative.as_str())),
    }
}
