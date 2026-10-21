use coffret_usecase::drop_walk::DroppedSource;

use super::DroppedFile;

/// The files one drop took, as the import that carries them in reads them.
///
/// Taken out of [`Drops`](super::Drops) once, by the import naming its handle;
/// nothing else holds it, so a second import naming the same handle finds
/// nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedSet {
    files: Vec<DroppedFile>,
}

impl DroppedSet {
    pub(super) fn new(taken: Vec<DroppedSource>) -> Self {
        Self {
            files: taken.into_iter().map(DroppedFile::new).collect(),
        }
    }

    /// The file at `index`, the position the page was told it at, or `None`
    /// where the drop holds no file there.
    pub fn file(&self, index: usize) -> Option<&DroppedFile> {
        self.files.get(index)
    }

    /// How many files the drop holds.
    pub fn len(&self) -> usize {
        self.files.len()
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}
