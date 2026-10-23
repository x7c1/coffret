use std::path::PathBuf;

use crate::local_error::LocalError;
use crate::local_io_error::LocalIoError;

/// What asking a mapped root which state it is in can fail with
/// ([`assess_root`](super::assess_root())).
///
/// Narrower than [`LocalError`], and on purpose: the root check lists one
/// folder and walks nothing, so it meets neither a name the Library cannot
/// hold nor two files claiming one Entry Path, and a caller holding this has
/// no arm to write for either.
#[derive(Debug)]
pub(crate) enum RootRefused {
    /// The root could not be probed or listed.
    Io(LocalIoError),
    /// A name in the root folds to the name reserved for the device's own
    /// management area without being it (spec: EP-14).
    FoldedReservedName {
        /// The folder or file whose name folds to the reserved one.
        path: PathBuf,
    },
}

impl From<LocalIoError> for RootRefused {
    /// What a local-filesystem capability reports, as the root check's own.
    fn from(error: LocalIoError) -> Self {
        Self::Io(error)
    }
}

impl From<RootRefused> for LocalError {
    /// The root check's refusal, in the vocabulary of the walk that asked it.
    fn from(error: RootRefused) -> Self {
        match error {
            RootRefused::Io(error) => Self::Io(error),
            RootRefused::FoldedReservedName { path } => Self::FoldedReservedName { path },
        }
    }
}
