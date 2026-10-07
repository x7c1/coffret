use std::error;

use super::IndexError;

impl error::Error for IndexError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::PendingRowsBusy { cause }
            | Self::UnreadableCatalog { cause, .. }
            | Self::Backend { cause, .. } => Some(cause.as_ref()),
            _ => None,
        }
    }
}
