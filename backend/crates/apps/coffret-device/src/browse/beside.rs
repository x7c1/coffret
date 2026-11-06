use std::cmp::Ordering;

use coffret_model::EntryPath;
use tracing::debug;

use super::FilesBeside;
use crate::error::Result;
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// The files either side of the row at `path` in reading order (see
    /// [`FilesBeside`]); `folder` says whether the row is the folder at that
    /// path rather than the file, since the Library admits both at one path.
    ///
    /// `under`, where given, limits the answer to the files standing under
    /// that folder, at any depth: the nearest of them either side of the row,
    /// each `None` where none of them stands that way. The row itself may
    /// stand anywhere — a row after everything under the folder answers with
    /// its last file as `previous`. `None` is the whole Library.
    ///
    /// A folder's place is just before everything under it, so the file after
    /// a folder is the first one inside it, at whatever depth, and the file
    /// before it is the last one before it. The row need not be an Entry the
    /// Library holds: a path names a place in the order whatever stands there.
    ///
    /// One read of the catalog — every current Entry, which the Library
    /// root's own listing reads as well, or only those under `under` — and
    /// nothing of Storage, so it answers for folders nobody has listed as
    /// readily as for those on the screen. One pass over the Entries, holding
    /// the nearest on either side.
    pub async fn files_beside(
        &self,
        path: &EntryPath,
        folder: bool,
        under: Option<&EntryPath>,
    ) -> Result<FilesBeside> {
        let read = self.index.entries_under(under).await?;
        // The catalog's prefix holds the Entry at the folder's own path too,
        // which is a file beside the folder rather than under it.
        let prefix = under.map(|under| format!("{}/", under.as_str()));
        let entries: Vec<_> = match &prefix {
            None => read,
            Some(prefix) => read
                .into_iter()
                .filter(|location| location.path().as_str().starts_with(prefix.as_str()))
                .collect(),
        };
        let at = Place::of(path, folder);
        let mut previous: Option<(&EntryPath, Place<'_>)> = None;
        let mut next: Option<(&EntryPath, Place<'_>)> = None;
        for location in &entries {
            let file = location.path();
            let place = Place::of(file, false);
            match place.cmp(&at) {
                Ordering::Less => {
                    if previous.as_ref().is_none_or(|(_, held)| place > *held) {
                        previous = Some((file, place));
                    }
                }
                Ordering::Greater => {
                    if next.as_ref().is_none_or(|(_, held)| place < *held) {
                        next = Some((file, place));
                    }
                }
                Ordering::Equal => {}
            }
        }
        debug!(
            operation = "files_beside",
            library = %self.library_id,
            entries = entries.len(),
            under = under.is_some(),
            previous = previous.is_some(),
            next = next.is_some(),
            "found the files either side of a row",
        );
        Ok(FilesBeside {
            previous: previous.map(|(file, _)| file.clone()),
            next: next.map(|(file, _)| file.clone()),
        })
    }
}

/// Where one row stands in reading order: its path's components, and whether
/// the last of them is a folder.
///
/// Compared component by component. A component that is a folder comes before
/// one that is a file, and two of the same kind compare by their bytes (spec:
/// EP-3); where one row's components run out first, it is the folder the other
/// stands under, and comes first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Place<'a> {
    path: &'a str,
    folder: bool,
}

impl<'a> Place<'a> {
    fn of(path: &'a EntryPath, folder: bool) -> Self {
        Self {
            path: path.as_str(),
            folder,
        }
    }

    /// Each component, with whether it is a file: every component but the
    /// last is a folder, and the last is a file unless the row is a folder.
    fn components(&self) -> impl Iterator<Item = (bool, &'a str)> + '_ {
        let count = self.path.split('/').count();
        self.path
            .split('/')
            .enumerate()
            .map(move |(at, name)| (at + 1 == count && !self.folder, name))
    }
}

impl Ord for Place<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut left = self.components();
        let mut right = other.components();
        loop {
            match (left.next(), right.next()) {
                (Some(one), Some(two)) => match one.cmp(&two) {
                    Ordering::Equal => {}
                    unequal => return unequal,
                },
                (None, Some(_)) => return Ordering::Less,
                (Some(_), None) => return Ordering::Greater,
                (None, None) => return Ordering::Equal,
            }
        }
    }
}

impl PartialOrd for Place<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
