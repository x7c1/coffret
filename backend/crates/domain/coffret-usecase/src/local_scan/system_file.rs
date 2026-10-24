use coffret_model::EntryPath;

use crate::system_files;

/// A name a scan left out because an operating system keeps it, where it would
/// otherwise have stood in the Library (spec: EP-17).
///
/// Nothing reports one. It is handed back so that a freeze's preview can say
/// what a freeze of a folder leaves as it is, beside the files it packs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SystemFile {
    /// The Entry Path it would have stood at.
    pub(crate) path: EntryPath,
    /// Whether a folder stood at the name, which the scan did not enter, so
    /// nothing under it is counted.
    pub(crate) folder: bool,
}

/// Whether the Entry Path `path`, under the mapping whose prefix is `prefix`,
/// is one a scan leaves out because an operating system keeps it
/// (spec: EP-17).
///
/// Asked of the components below the mapped root, since a volume's own folders
/// are left out only as the first of them, and a top-level mapping's prefix is
/// not a folder on this disk.
///
/// A scan never finds a file at such a path, so an Entry standing there —
/// carried in before the rule — is not this device's file gone: a run asks
/// this before reading the absence as a deletion, or as an Entry whose file
/// this device has not got.
pub(crate) fn kept_by_system(prefix: Option<&EntryPath>, path: &EntryPath) -> bool {
    let below = path.as_str().split('/').skip(usize::from(prefix.is_some()));
    system_files::system_component(below).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_paths::entry_path;

    // EP-17: a volume's folder counts only as the first component below the
    // mapped root, which under a top-level mapping is the second of the path.
    #[test]
    fn a_path_is_judged_below_the_mapped_root() {
        let albums = entry_path("albums");
        assert!(kept_by_system(None, &entry_path("book/.DS_Store")));
        assert!(kept_by_system(None, &entry_path(".Spotlight-V100/x")));
        assert!(!kept_by_system(
            None,
            &entry_path("albums/.Spotlight-V100/x")
        ));
        assert!(kept_by_system(
            Some(&albums),
            &entry_path("albums/.Spotlight-V100/x")
        ));
        assert!(kept_by_system(
            Some(&albums),
            &entry_path("albums/a/._1.jpg")
        ));
        assert!(!kept_by_system(
            Some(&albums),
            &entry_path("albums/a/1.jpg")
        ));
    }
}
