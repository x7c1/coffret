use coffret_model::EntryPath;

use super::{ChildFolder, FileRow};

/// What one folder of the Library holds, one level down.
///
/// A Library has no folders of its own: an Entry Path is one string and `/` is
/// the only logical separator in it (spec: EP-2), so a folder is what the
/// separator implies rather than something the catalog stores. This is that
/// implication made explicit for one folder — the child folders and the child
/// files — and nothing under them.
///
/// Both lists are in EP-3 order: the byte order of the canonical paths, with no
/// case folding and no locale. That is the order the catalog answered in, and it
/// is the order because it is the only one every device agrees on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderListing {
    /// The folder this is a listing of, or `None` for the Library root.
    pub path: Option<EntryPath>,
    /// Whether a mapping of this device reaches this folder (spec: EP-9).
    ///
    /// It says nothing about what is on disk — that is each row's
    /// [`state`](FileRow::state) and only a materialization record answers it
    /// (spec: EP-10) — and everything about whether anything here *can* be. A
    /// folder no mapping reaches has nowhere on this device to put a file, so
    /// every fetch under it would be declined, and the mappings say so before a
    /// reader asks for one rather than after a round trip to Storage.
    ///
    /// The Library root is reached by a root mapping alone: a top-level mapping
    /// stands for its own subtree and not for what sits beside it.
    pub mapped: bool,
    /// Whether the Library has this folder at all.
    ///
    /// A folder is what the separators in the Entry Paths under it imply, so it
    /// is there because something is under it and it stops being there when the
    /// last of that goes. Which means a folder of the Library is never empty,
    /// and an empty listing of one is not an empty folder but a path the Library
    /// names nothing at: a component mistyped, or a link kept past the Entries
    /// it was about.
    ///
    /// The two are the same rows — none — so nothing about the lists below tells
    /// them apart, and this is the field that does. The Library root is the one
    /// folder that is there without anything under it: it is the Library rather
    /// than something a path implies, so a Library holding nothing still has
    /// one and this says `true` of it.
    pub held: bool,
    /// The folders directly inside it.
    pub folders: Vec<ChildFolder>,
    /// The files directly inside it.
    pub files: Vec<FileRow>,
}
