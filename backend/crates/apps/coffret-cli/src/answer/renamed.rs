//! What `rename` answers with.

use serde::Serialize;

use coffret_device::RenameOutcome;

use super::committed_head;

/// What `rename` did.
#[derive(Serialize)]
pub struct Renamed {
    renamed: usize,
    folder: bool,
    committed_head: Option<u64>,
}

impl Renamed {
    /// The answer of a rename of `renamed` Entries committed at
    /// `committed_head`, for the golden cases, which cannot assemble the commit
    /// a whole outcome carries.
    #[cfg(test)]
    pub(super) fn assembled(renamed: usize, folder: bool, committed_head: u64) -> Self {
        Self {
            renamed,
            folder,
            committed_head: Some(committed_head),
        }
    }
}

impl From<&RenameOutcome> for Renamed {
    fn from(outcome: &RenameOutcome) -> Self {
        Self {
            renamed: outcome.renamed.len(),
            folder: outcome.folder,
            committed_head: committed_head(Some(&outcome.commit)),
        }
    }
}
