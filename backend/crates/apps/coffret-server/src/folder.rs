use coffret_device::EntryPath;

/// One folder of the Library, as the work over one names it.
///
/// `None` is the Library root, which is not an Entry Path at all (spec: EP-2)
/// and is spelled as the empty string wherever it goes on the wire — the same
/// spelling a listing's own `path` uses, so a browser comparing the two is
/// comparing like with like.
///
/// Shared by the two pieces of background work that are *of a folder* — the
/// fill that brings one over and the freeze that packs one (spec: PK-17) —
/// rather than owned by either. Both name a place in the Library the same way,
/// and two values for it would be two spellings of the Library root to keep in
/// step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Folder(Option<EntryPath>);

impl Folder {
    /// The folder holding one Entry.
    ///
    /// The path's own answer, which is the whole of this: an Entry Path's only
    /// logical separator is `/` (spec: EP-2), so everything before the last one
    /// is the folder and a path with none of them sits directly in the Library
    /// root.
    pub fn holding(entry: &EntryPath) -> Self {
        Self(entry.parent())
    }

    /// The folder a caller named, `None` being the Library root.
    pub fn named(path: Option<EntryPath>) -> Self {
        Self(path)
    }

    /// Its path, as the wire spells it.
    pub fn as_str(&self) -> &str {
        self.0.as_ref().map_or("", EntryPath::as_str)
    }

    /// The folder as [`list`](coffret_device::OpenLibrary::list) takes it, and
    /// as [`freeze`](coffret_device::OpenLibrary::freeze) takes its prefix
    /// (spec: PK-17).
    pub(crate) fn listed(&self) -> Option<&EntryPath> {
        self.0.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::Folder;
    use crate::entry_paths::entry_path;

    #[test]
    fn a_folder_is_everything_before_an_entrys_last_separator() {
        assert_eq!(
            Folder::holding(&entry_path("albums/2026/spring.jpg")).as_str(),
            "albums/2026",
        );
        assert_eq!(
            Folder::holding(&entry_path("albums/cover.png")).as_str(),
            "albums",
        );
    }

    // An Entry sitting directly in the Library root is in no folder at all, and
    // the root is the empty string rather than a path (spec: EP-2).
    #[test]
    fn an_entry_with_no_separator_is_in_the_library_root() {
        let root = Folder::holding(&entry_path("notes.txt"));
        assert_eq!(root.as_str(), "");
        assert_eq!(root, Folder::named(None));
        assert_eq!(root.listed(), None);
    }
}
