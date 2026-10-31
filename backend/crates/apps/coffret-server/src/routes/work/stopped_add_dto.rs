use serde::Serialize;

use crate::adds::StoppedAdd;

/// An add a person stopped, as a page is told what undoing it came to
/// (spec: LA-36).
#[derive(Serialize)]
pub(super) struct StoppedAddDto {
    /// The folder it was made onto, as a listing spells it.
    folder: String,
    /// How many files it had written.
    files: usize,
    /// How many of them were taken away from the folder again: removed, or
    /// replaced by the file they had been written over.
    removed: usize,
    /// How many a run had committed before the stop was answered.
    committed: usize,
    /// How many are still in the folder for any other reason.
    left: usize,
}

impl StoppedAddDto {
    pub(super) fn of(stopped: &StoppedAdd) -> Self {
        Self {
            folder: stopped.folder.as_str().to_owned(),
            files: stopped.files,
            removed: stopped.removed,
            committed: stopped.committed,
            left: stopped.left,
        }
    }
}
