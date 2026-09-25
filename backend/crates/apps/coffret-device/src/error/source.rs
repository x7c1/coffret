//! Which failure each [`Error`] variant carries under its own line, if any.

use std::error;

use coffret_usecase::RootRefused;

use super::Error;

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::InvalidLibraryName { .. }
            | Self::NoStateDirectory
            | Self::LibraryExists { .. }
            | Self::NoSuchLibrary { .. }
            | Self::NotADriveLibrary { .. }
            | Self::LibraryAlreadyServed { .. }
            | Self::ManagementAreaNotADirectory { .. }
            | Self::ManagementAreaIncomplete { .. }
            | Self::ManagementAreaFolded { .. }
            | Self::MarkerNotARegularFile { .. }
            | Self::UnsupportedSettingsVersion { .. } => None,
            Self::MarkerMalformed { cause, .. } => Some(cause),
            // The marker's own refusal where that is what made it, so a printed
            // chain ends at what the file held rather than at the root.
            Self::RootRefused(refusal) => match &refusal.reason {
                RootRefused::MarkerMalformed { cause } => Some(cause),
                _ => None,
            },
            Self::RootMarkerNotDrawn { cause, .. } => Some(cause),
            Self::ServerKeyNotDrawn { cause } => Some(cause),
            Self::Local(refused) | Self::RootUnvouched { cause: refused, .. } => {
                Some(&refused.cause)
            }
            Self::MalformedSettings { cause, .. } | Self::UnencodableSettings { cause, .. } => {
                Some(cause)
            }
            Self::MasterKeyNotUnlocked { cause, .. } | Self::KeyMaterial { cause } => Some(cause),
            Self::MalformedStoragePrefix { cause } => Some(cause),
            // The model's refusal where the prefix is no Entry Path, and nothing
            // underneath where it is one and names more than one component.
            Self::MalformedMappingPrefix { cause, .. } => cause
                .as_ref()
                .map(|cause| cause as &(dyn error::Error + 'static)),
            Self::Index { cause } => Some(cause),
            Self::Drive { cause } => Some(cause.as_ref()),
            Self::NotAuthorized { cause, .. } => cause
                .as_ref()
                .map(|cause| cause.as_ref() as &(dyn error::Error + 'static)),
            Self::NoSuchLocalRoot { cause, .. } => cause
                .as_ref()
                .map(|cause| cause as &(dyn error::Error + 'static)),
            Self::PassphraseNotGiven { cause } | Self::RecoveryCodeNotGiven { cause } => {
                Some(cause.as_ref())
            }
            Self::BucketUnreachable { cause, .. } => Some(cause),
            Self::MalformedRecoveryCode { cause } => Some(cause),
            Self::NotALibraryFolder { cause, .. } => cause
                .as_ref()
                .map(|cause| cause as &(dyn error::Error + 'static)),
            Self::Sync { cause } => Some(cause.as_ref()),
            Self::Freeze { cause } => Some(cause.as_ref()),
            Self::Fetch { cause } => Some(cause.as_ref()),
            Self::LocalPathNotSettled { cause }
            | Self::FileNotTakenIn { cause }
            | Self::LocalFilesNotRead { cause }
            | Self::LocalFileNotOpened { cause } => Some(cause.as_ref()),
            Self::CatchUp { cause } => Some(cause.as_ref()),
            Self::LibraryNotCreated { cause, .. } | Self::LibraryNotJoined { cause, .. } => {
                Some(cause.as_ref())
            }
        }
    }
}
