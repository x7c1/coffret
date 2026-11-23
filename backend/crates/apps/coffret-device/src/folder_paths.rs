use coffret_model::{EntryPath, Result};

/// Where a child of `folder` called `name` stands in the Library, or a refusal
/// where `name` is no name the Library can hold.
///
/// The name comes off a filesystem, so it is whichever spelling that platform
/// kept and whatever characters that platform allows — it is read as the
/// one-component path it has to be (spec: EP-1, EP-2), and the folder above it,
/// already a path, is what it is joined under. The join owes no reading of its
/// own, which is why only the name has one.
///
/// # Errors
///
/// The model's own refusal where `name` is no component of an Entry Path, which
/// on this side of the boundary means a local file the Library has no position
/// for.
pub(crate) fn child_path(folder: Option<&EntryPath>, name: &str) -> Result<EntryPath> {
    let name = EntryPath::parse(name)?;
    Ok(match folder {
        None => name,
        Some(folder) => folder.below(&name),
    })
}
