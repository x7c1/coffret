use std::fmt;

/// Why a record committed since the batch was prepared and one of the batch's
/// pending changes cannot both stand (spec: CP-7, EP-7).
///
/// Every one of them is a write to one Entry or one Entry Path from two sides,
/// which is never resolved by picking one (spec: CP-7). The ones a rebase
/// carries over instead — an update or a rebuilt Pack taking over an Entry at
/// the path a rename gave it, a rename following the Entry an update replaced
/// it with, a removal of an Entry renamed meanwhile — are not here, because
/// nothing about them conflicts.
#[derive(Debug, Clone, Copy)]
pub enum RebaseRule {
    /// The record renamed another Entry onto the path the addition places its
    /// Entry at (spec: EP-7, rule 1).
    RenamedOntoAddedPath,
    /// The record removed the Container of the Entry the rename was prepared
    /// for, and the name the rename was prepared against names no Entry that
    /// record added in its place (spec: EP-7, rule 3).
    RenamedEntryReplaced,
    /// The record renamed the same Entry (spec: EP-7, rule 4).
    RenamedEntryRenamed,
    /// The record added or renamed an Entry onto the rename's destination, and
    /// that Entry still holds it (spec: EP-7, rule 5).
    DestinationTaken,
}

impl fmt::Display for RebaseRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::RenamedOntoAddedPath => "gave another Entry that path by a rename",
            Self::RenamedEntryReplaced => {
                "replaced its Container, and no Entry of the replacement stands at the name \
                 the rename was prepared against"
            }
            Self::RenamedEntryRenamed => "renamed the same Entry",
            Self::DestinationTaken => "put another Entry at the path the rename gives it",
        })
    }
}
