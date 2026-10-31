use std::collections::BTreeSet;

use coffret_model::EntryPath;

/// Which of the mapped folders one sync walks (spec: EP-10).
///
/// A sync covers every mapped folder, which is what the command line runs and
/// what every sync that no drop armed runs. A sync a drop armed covers the
/// folders the drop wrote its files into and nothing else (spec: LA-34): the
/// files standing directly in each of them, and no folder beside or below
/// one. What lies outside is not looked at at all — not walked, not read as
/// deleted, not read as departed — so it waits, as it is, for a sync that
/// covers it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ScanScope {
    /// Every folder the device maps (spec: EP-9).
    #[default]
    Mappings,
    /// The folders a drop wrote files into, each by its Entry Path.
    Folders {
        /// Whether the Library root is among them: a file dropped there
        /// stands directly in it.
        root: bool,
        /// The other folders, each the folder a dropped file stands in.
        folders: BTreeSet<EntryPath>,
    },
}

impl ScanScope {
    /// The folders `files` stand in: what a sync carrying those files in
    /// walks, and all it walks.
    pub fn folders_of<'a>(files: impl IntoIterator<Item = &'a EntryPath>) -> Self {
        let mut root = false;
        let mut folders = BTreeSet::new();
        for file in files {
            match file.parent() {
                Some(folder) => {
                    folders.insert(folder);
                }
                None => root = true,
            }
        }
        Self::Folders { root, folders }
    }

    /// Whether this is every mapped folder.
    pub fn is_mappings(&self) -> bool {
        matches!(self, Self::Mappings)
    }

    /// Whether the files standing directly in `folder` — the Library root
    /// where it is `None` — are in scope.
    pub(crate) fn lists(&self, folder: Option<&EntryPath>) -> bool {
        match self {
            Self::Mappings => true,
            Self::Folders { root, folders } => match folder {
                None => *root,
                Some(folder) => folders.contains(folder),
            },
        }
    }

    /// Whether a walk enters `folder`: it is in scope, or a folder in scope
    /// lies below it.
    pub(crate) fn enters(&self, folder: &EntryPath) -> bool {
        match self {
            Self::Mappings => true,
            Self::Folders { folders, .. } => folders.iter().any(|one| one.is_under(folder)),
        }
    }

    /// Whether the file at `path` stands in a folder in scope.
    pub(crate) fn covers(&self, path: &EntryPath) -> bool {
        self.lists(path.parent().as_ref())
    }

    /// Whether a mapping of `prefix` reaches anything in scope, given the
    /// top-level components other mappings stand for, which a Library-root
    /// mapping does not reach (spec: EP-9).
    pub(crate) fn reaches(&self, prefix: Option<&EntryPath>, elsewhere: &BTreeSet<&str>) -> bool {
        match (self, prefix) {
            (Self::Mappings, _) => true,
            (Self::Folders { .. }, Some(prefix)) => self.enters(prefix),
            (Self::Folders { root, folders }, None) => {
                *root
                    || folders
                        .iter()
                        .any(|folder| !elsewhere.contains(folder.top_level()))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::ScanScope;
    use crate::entry_paths::entry_path;

    #[test]
    fn a_drop_covers_the_folders_its_files_stand_in_and_nothing_beside_or_below() {
        let scope = ScanScope::folders_of(&[
            entry_path("albums/2026/spring.jpg"),
            entry_path("albums/2026/summer.jpg"),
        ]);

        assert!(scope.covers(&entry_path("albums/2026/autumn.jpg")));
        assert!(!scope.covers(&entry_path("albums/cover.png")));
        assert!(!scope.covers(&entry_path("albums/2026/old/winter.jpg")));
        assert!(!scope.covers(&entry_path("top.jpg")));
        assert!(scope.enters(&entry_path("albums")));
        assert!(scope.enters(&entry_path("albums/2026")));
        assert!(!scope.enters(&entry_path("albums/2026/old")));
        assert!(!scope.enters(&entry_path("books")));
        assert!(!scope.lists(None));
    }

    #[test]
    fn a_mapping_is_reached_only_where_a_folder_in_scope_lies_under_it() {
        let scope = ScanScope::folders_of(&[entry_path("albums/a.jpg")]);
        let elsewhere = BTreeSet::from(["albums"]);

        assert!(scope.reaches(Some(&entry_path("albums")), &elsewhere));
        assert!(!scope.reaches(Some(&entry_path("books")), &elsewhere));
        assert!(
            !scope.reaches(None, &elsewhere),
            "the root mapping does not stand for a subtree another mapping does",
        );
        assert!(scope.reaches(None, &BTreeSet::new()));
        assert!(ScanScope::folders_of(&[entry_path("top.jpg")]).reaches(None, &elsewhere));
    }
}
