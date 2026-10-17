use std::fmt;

use crate::commit::entry_ref::EntryRef;

/// Which of a batch's pending changes a rebase could not carry onto the new
/// head (spec: EP-7).
#[derive(Debug, Clone, Copy)]
pub enum PendingChange {
    /// One Entry of a Container the batch adds, named by that Container and
    /// the Entry's number in it.
    Addition(EntryRef),
    /// A rename, named by the Entry it was prepared to rename.
    Rename(EntryRef),
}

impl PendingChange {
    /// The change's kind and the Entry it names, and nothing anybody chose:
    /// what a diagnostic event may carry (spec: EL-1).
    pub(super) fn redacted(&self) -> String {
        match self {
            Self::Addition(entry) => format!(
                "Addition(container={}, entry={})",
                entry.container_id, entry.entry_number
            ),
            Self::Rename(entry) => format!(
                "Rename(container={}, entry={})",
                entry.container_id, entry.entry_number
            ),
        }
    }
}

impl fmt::Display for PendingChange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Addition(entry) => write!(f, "the batch's addition of {entry}"),
            Self::Rename(entry) => write!(f, "the batch's rename of {entry}"),
        }
    }
}
