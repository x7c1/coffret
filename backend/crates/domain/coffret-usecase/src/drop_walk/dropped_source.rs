use std::path::PathBuf;

use coffret_model::{Btime, Mtime};

use crate::mapped_relative_location::MappedRelativeLocation;

/// One regular file the walk took.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedSource {
    /// The folder the dropped item it was found under stood in, which is the
    /// root it is read from again.
    pub root: PathBuf,
    /// Where it is below that root, spelled as the folder holds it.
    pub relative: MappedRelativeLocation,
    /// The same, as text: `photo.jpg` for a file dropped on its own,
    /// `holiday/day1/photo.jpg` for one inside a dropped folder — what the
    /// browser's walk calls the same file.
    pub path: String,
    /// Its length when the walk stated it.
    pub size: u64,
    /// Its modification time when the walk stated it (spec: FM-9).
    pub mtime: Mtime,
    /// Its birth time, where the platform reports one (spec: FM-9).
    pub btime: Option<Btime>,
}
