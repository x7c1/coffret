use std::cmp::Ordering;

use coffret_model::EntryPath;
use coffret_usecase::folder_figures::Side;
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
    /// Read out of the catalog alone, and nothing of Storage, so it answers
    /// for folders nobody has listed as readily as for those on the screen.
    /// What it reads is the row's own folders and no more: from the folder
    /// the row stands in up towards `under` (or the root), the nearest child
    /// folder or Entry on the side it looks, one at a time, and then down the
    /// first or last child of each folder into the file it answers with —
    /// a few rows per level of the path, however large the Library is.
    pub async fn files_beside(
        &self,
        path: &EntryPath,
        folder: bool,
        under: Option<&EntryPath>,
    ) -> Result<FilesBeside> {
        let previous = self.file_beside(path, folder, under, Side::Before).await?;
        let next = self.file_beside(path, folder, under, Side::After).await?;
        debug!(
            operation = "files_beside",
            library = %self.library_id,
            under = under.is_some(),
            previous = previous.is_some(),
            next = next.is_some(),
            "found the files either side of a row",
        );
        Ok(FilesBeside { previous, next })
    }

    /// The nearest file on `side` of the row, among those under `under`.
    async fn file_beside(
        &self,
        path: &EntryPath,
        folder: bool,
        under: Option<&EntryPath>,
        side: Side,
    ) -> Result<Option<EntryPath>> {
        if let Some(under) = under {
            if path == under || !path.is_under(under) {
                // Everything under the folder is one stretch of the order —
                // the stretch just after its own place — so a row outside it
                // has the whole of it on one side and none on the other. The
                // file at the folder's own path stands after it, and the
                // folder itself just before it.
                let before_it = Place::of(path, folder) <= Place::of(under, true);
                return Ok(match (side, before_it) {
                    (Side::After, true) => self.end_of(under, Side::After).await?,
                    (Side::Before, false) => self.end_of(under, Side::Before).await?,
                    _ => None,
                });
            }
        }
        // A folder's place is just before everything under it, so its first
        // file is the next one where it holds any.
        if folder && side == Side::After {
            if let Some(first) = self.end_of(path, Side::After).await? {
                return Ok(Some(first));
            }
        }
        let mut bound = path.clone();
        let mut bound_is_folder = folder;
        loop {
            let parent = bound.parent();
            if let Some(found) = self
                .beside_in(parent.as_ref(), &bound, bound_is_folder, side)
                .await?
            {
                return Ok(Some(found));
            }
            match parent {
                // Climbing stops at the folder the answer is limited to.
                Some(parent) if Some(&parent) != under => {
                    bound = parent;
                    bound_is_folder = true;
                }
                _ => return Ok(None),
            }
        }
    }

    /// The nearest file on `side` of `bound` among what stands in `folder`,
    /// at any depth: `bound` stands directly in it, and is a folder where
    /// `bound_is_folder` says so.
    ///
    /// A folder's folders come before its files, so from a folder the files
    /// after it are those in the folders after it and then the folder's own
    /// files, and those before it are in the folders before it alone; from a
    /// file, the files after it are the folder's later files alone, and those
    /// before it are its earlier files and then everything in its folders.
    async fn beside_in(
        &self,
        folder: Option<&EntryPath>,
        bound: &EntryPath,
        bound_is_folder: bool,
        side: Side,
    ) -> Result<Option<EntryPath>> {
        match (bound_is_folder, side) {
            (true, Side::After) => {
                if let Some(found) = self.in_child_folders(folder, Some(bound), side).await? {
                    return Ok(Some(found));
                }
                Ok(self.index.entry_beside(folder, None, side).await?)
            }
            (true, Side::Before) => self.in_child_folders(folder, Some(bound), side).await,
            (false, Side::After) => Ok(self.index.entry_beside(folder, Some(bound), side).await?),
            (false, Side::Before) => {
                if let Some(found) = self.index.entry_beside(folder, Some(bound), side).await? {
                    return Ok(Some(found));
                }
                self.in_child_folders(folder, None, side).await
            }
        }
    }

    /// The nearest file inside the folders standing directly in `folder` on
    /// `side` of `bound`: the first file of the first of them after it, or
    /// the last file of the last of them before it.
    ///
    /// Every folder of the Library has a current Entry under it (spec: EP-2),
    /// so the nearest folder holds the answer and, as the catalog stands, no
    /// other is read. These are several reads rather than one, though, and a
    /// commit between two of them may empty the folder found; the next one
    /// over is then read, rather than every folder past it being passed over.
    async fn in_child_folders(
        &self,
        folder: Option<&EntryPath>,
        bound: Option<&EntryPath>,
        side: Side,
    ) -> Result<Option<EntryPath>> {
        let mut bound = bound.cloned();
        while let Some(child) = self
            .index
            .folder_beside(folder, bound.as_ref(), side)
            .await?
        {
            if let Some(found) = Box::pin(self.end_of(&child, side)).await? {
                return Ok(Some(found));
            }
            bound = Some(child);
        }
        Ok(None)
    }

    /// The first file under `folder` in reading order, at any depth, or the
    /// last: the first file inside its first folder, and where it has none,
    /// its own first file; or its own last file, and where it has none, the
    /// last file inside its last folder.
    async fn end_of(&self, folder: &EntryPath, side: Side) -> Result<Option<EntryPath>> {
        match side {
            Side::After => {
                if let Some(found) = self.in_child_folders(Some(folder), None, side).await? {
                    return Ok(Some(found));
                }
                Ok(self.index.entry_beside(Some(folder), None, side).await?)
            }
            Side::Before => {
                if let Some(found) = self.index.entry_beside(Some(folder), None, side).await? {
                    return Ok(Some(found));
                }
                self.in_child_folders(Some(folder), None, side).await
            }
        }
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
