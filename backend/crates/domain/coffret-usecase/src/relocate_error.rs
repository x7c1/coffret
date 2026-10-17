use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use coffret_model::Redacted;

use crate::descent_error::DescentError;

/// Why a file could not be relocated to follow its Entry (spec: EP-10).
///
/// The vocabulary of [`Relocations`](crate::Relocations): the descent's own
/// refusals, and the three answers only a rename gives. Which errno stood behind
/// any of them is the gateway's to read and never a caller's.
///
/// There is deliberately no `PartialEq`, for the reason the error types around
/// it have none.
#[derive(Debug)]
pub enum RelocateError {
    /// The root would not vouch for itself, a name on the way to either path is
    /// not a real folder of the root, or the disk refused (spec: EP-8, EP-13).
    Descent(DescentError),
    /// Something already stands where the file was to go, and it is never
    /// replaced.
    Taken {
        /// Where it stands, for the caller that names it — never for a
        /// diagnostic event (spec: EL-1).
        at: PathBuf,
    },
    /// The two names stand on two filesystems, so no rename joins them, and
    /// nothing is copied in its place.
    AnotherFilesystem {
        /// Where the file stands.
        from: PathBuf,
        /// Where it was to go.
        to: PathBuf,
    },
    /// The filesystem offers no rename that never replaces what stands at the
    /// new name, or none that exchanges two names in one step — as FAT volumes
    /// and network shares can — and nothing weaker is made in its place: asking
    /// whether the name is free and then renaming onto it leaves a window a
    /// file arriving in it would be overwritten in.
    ///
    /// Not a passing refusal: the same filesystem answers the same way every
    /// run.
    NoAtomicRename {
        /// Where the rename was asked for, for the caller that names it —
        /// never for a diagnostic event (spec: EL-1).
        at: PathBuf,
        /// What the operating system answered the rename with.
        cause: io::Error,
    },
}

impl fmt::Display for RelocateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Descent(_) => {
                f.write_str("the file could not be relocated inside the mapped root")
            }
            Self::Taken { .. } => {
                f.write_str("something already stands where the file was to be relocated")
            }
            Self::AnotherFilesystem { .. } => f.write_str(
                "where the file stands and where it was to go are on two filesystems, and \
                 nothing is copied across",
            ),
            Self::NoAtomicRename { .. } => f.write_str(
                "the filesystem offers no rename that leaves what stands at the new name untouched",
            ),
        }
    }
}

impl error::Error for RelocateError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Descent(refused) => Some(refused),
            Self::NoAtomicRename { cause, .. } => Some(cause),
            Self::Taken { .. } | Self::AnotherFilesystem { .. } => None,
        }
    }
}

impl Redacted for RelocateError {
    /// Which refusal it is, and the descent's own log-safe form where that is
    /// what refused: the paths stay in the value (spec: EL-1).
    fn redacted(&self) -> String {
        match self {
            Self::Descent(refused) => format!("Relocate::{}", refused.redacted()),
            Self::Taken { .. } => "Relocate::Taken".to_owned(),
            Self::AnotherFilesystem { .. } => "Relocate::AnotherFilesystem".to_owned(),
            Self::NoAtomicRename { cause, .. } => {
                format!("Relocate::NoAtomicRename(kind={:?})", cause.kind())
            }
        }
    }
}

impl From<DescentError> for RelocateError {
    fn from(refused: DescentError) -> Self {
        Self::Descent(refused)
    }
}

impl From<crate::local_io_error::LocalIoError> for RelocateError {
    /// What the operating system refused, as the descent carries it.
    fn from(refused: crate::local_io_error::LocalIoError) -> Self {
        Self::Descent(DescentError::Io(refused))
    }
}
