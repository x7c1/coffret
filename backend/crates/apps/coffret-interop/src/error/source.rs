//! What each failure says underneath: the value the layer below reported, or
//! the failure a variant that says where wraps.

use std::error;

use super::Error;

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Read { cause, .. }
            | Self::Write { cause, .. }
            | Self::CreateDirectory { cause, .. } => Some(cause),
            Self::ManifestUnparsed { cause, .. }
            | Self::ManifestUnserialized { cause }
            | Self::DumpUnrendered { cause } => Some(cause),
            Self::StatedHex { cause, .. } => Some(cause),
            Self::StatedValue { cause, .. } | Self::Model { cause, .. } => Some(cause),
            Self::Format { cause, .. } | Self::KeyringUnopened { cause, .. } => Some(cause),
            Self::RecoveryCodeNotText { cause } => Some(cause),
            Self::BodyUnreadable { cause } => Some(cause),
            Self::InSet { cause, .. }
            | Self::InFixture { cause, .. }
            | Self::InEntry { cause, .. } => Some(cause.as_ref()),
            Self::OutsideSet { .. }
            | Self::UnknownSchema { .. }
            | Self::FixtureUnlisted { .. }
            | Self::Differs { .. }
            | Self::SetDigestDiffers { .. }
            | Self::NoSetDigest
            | Self::EpochDiffers { .. }
            | Self::NotAMap { .. }
            | Self::FieldCount { .. }
            | Self::NoField { .. }
            | Self::NotAnArray { .. }
            | Self::ElementCount { .. }
            | Self::NoKeyOpens { .. }
            | Self::ContainerNeedsKeyring { .. }
            | Self::KeyringNameNotUnicode { .. }
            | Self::NothingForContainer { .. }
            | Self::ContainerKeyLost { .. }
            | Self::ShorterThanHeader { .. }
            | Self::ShorterThanMetaSection { .. }
            | Self::UnnamableKey { .. }
            | Self::UnrenderableValue { .. } => None,
        }
    }
}

impl Error {
    /// Every link of the chain, outermost first, each as it says itself.
    ///
    /// What `main` prints on a failure, and what a case reads a lower layer's
    /// answer from.
    pub fn chain(&self) -> Vec<String> {
        let mut links = vec![self.to_string()];
        let mut below = error::Error::source(self);
        while let Some(link) = below {
            links.push(link.to_string());
            below = link.source();
        }
        links
    }
}
