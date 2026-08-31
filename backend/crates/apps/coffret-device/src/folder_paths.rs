use coffret_model::EntryPath;

/// What is left of an Entry Path once the folder above it is taken off, or
/// `None` where the path is the folder itself.
///
/// The separator is stripped along with the prefix, which is what keeps a
/// sibling whose name merely starts with the same letters out of the folder
/// (spec: EP-2, EP-9) — though a catalog range narrowed to the folder has
/// already excluded it.
pub(crate) fn inside<'a>(folder: Option<&EntryPath>, path: &'a EntryPath) -> Option<&'a str> {
    match folder {
        None => Some(path.as_str()),
        Some(folder) => path
            .as_str()
            .strip_prefix(folder.as_str())
            .and_then(|rest| rest.strip_prefix('/')),
    }
}

/// Where a child of `folder` called `name` stands in the Library.
///
/// The name may come from the catalog, where it is already the Library's
/// spelling, or off a filesystem, where it is whichever spelling that platform
/// kept — so it is composed here (spec: EP-1). Composing what is already
/// composed changes nothing, which is what lets one function answer for both.
pub(crate) fn child_path(folder: Option<&EntryPath>, name: &str) -> EntryPath {
    match folder {
        None => EntryPath::nfc(name),
        Some(folder) => EntryPath::nfc(format!("{}/{name}", folder.as_str())),
    }
}
