use crate::commit::CommitOutcome;
use crate::relocate::Relocation;
use crate::rename::renamed_path::RenamedPath;

/// What a rename did.
#[derive(Debug)]
pub struct RenameOutcome {
    /// Every Entry the rename gave a new path, ordered by the path it left.
    ///
    /// As planned against the head the run caught up to. A rebase onto a
    /// record another device committed meanwhile can carry a rename over to the
    /// Entry that replaced the one planned (spec: EP-7); the record in
    /// [`commit`](Self::commit) is what the Library holds.
    pub renamed: Vec<RenamedPath>,
    /// Whether the source named a folder — Entries under it — rather than one
    /// Entry alone.
    pub folder: bool,
    /// The commit the renames landed in (spec: CP-1).
    pub commit: CommitOutcome,
    /// What became of this device's files of the renamed Entries, where the run
    /// was handed the mapped folders to relocate them in (spec: EP-10).
    ///
    /// Empty where it was not, and where this device holds none of them.
    pub relocations: Vec<Relocation>,
}
