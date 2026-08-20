use std::error;
use std::fmt;
use std::time::Duration;

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything an [`ObjectStore`](crate::ObjectStore) operation can fail with.
///
/// The variants are the storage vocabulary the use-case layer reasons in, not
/// any one provider's error catalogue: a gateway translates whatever its SDK or
/// its HTTP responses report into these, so a caller never inspects a provider
/// message to decide what happened. Two distinctions the layer above depends on
/// are therefore carried by the type itself:
///
/// - [`Error::AlreadyExists`] is the lost conditional create — the commit slot
///   was consumed by someone else — and is never raised for a transport
///   failure that merely might have created the object.
/// - [`Error::is_retryable`] separates failures that a later identical attempt
///   can still succeed at from ones that never will, so a retry loop needs no
///   string matching.
#[derive(Debug, Clone)]
pub enum Error {
    /// No object exists under the name or reference the operation names.
    NotFound {
        /// The object the operation asked for.
        object: String,
    },
    /// A conditional create found the slot already taken.
    ///
    /// The commit protocol turns this into "another writer committed first":
    /// refresh the head and retry, never overwrite.
    AlreadyExists {
        /// The object the create would have written.
        object: String,
    },
    /// The credentials are valid but do not authorize this operation.
    PermissionDenied {
        /// What the provider reported.
        detail: String,
    },
    /// The credentials are missing, expired beyond refresh, or rejected.
    Unauthenticated {
        /// What the provider reported.
        detail: String,
    },
    /// The provider's own digest of the stored bytes disagrees with the digest
    /// computed while uploading them.
    ///
    /// The object reached Storage corrupted, or not all of it arrived; either
    /// way the upload has not succeeded.
    IntegrityMismatch {
        /// The digest computed locally over the bytes that were sent.
        expected: String,
        /// The digest the provider reports for what it stored.
        actual: String,
    },
    /// A [`purge`](crate::ObjectStore::purge) deleted the object but a read-back
    /// still found it.
    ///
    /// Purge is the irreversible removal that Master Key rotation depends on,
    /// so an unconfirmed deletion is a failure rather than a warning.
    NotPurged {
        /// The object that survived its deletion.
        object: String,
    },
    /// The store cannot carry out the operation as asked — a commit slot minted
    /// by a different store, or an object name it has no way to represent.
    Unsupported {
        /// What about the request the store cannot honour.
        detail: String,
    },
    /// The provider refused the request for a reason none of the other variants
    /// name, and repeating it unchanged would be refused again.
    ///
    /// A gateway reaches for this only where the provider's answer maps to no
    /// state the port knows; it is a permanent failure, so a caller reports it
    /// rather than looping on it.
    Rejected {
        /// The HTTP status the provider answered with.
        status: u16,
        /// What the provider reported.
        detail: String,
    },
    /// The provider answered with something this build cannot read.
    MalformedResponse {
        /// What went wrong reading the response.
        detail: String,
    },
    /// A stream carried fewer or more bytes than its declared length.
    LengthMismatch {
        /// The length the stream declared.
        expected: u64,
        /// The length actually transferred.
        actual: u64,
    },
    /// Reading or writing the local end of a transfer failed.
    Io {
        /// What the operating system reported.
        detail: String,
    },
    /// The provider is refusing calls for now and names how long to wait.
    RateLimited {
        /// How long the provider asks the caller to wait, when it says.
        retry_after: Option<Duration>,
        /// What the provider reported.
        detail: String,
    },
    /// The provider failed on its own side.
    ServiceUnavailable {
        /// The HTTP status the provider answered with.
        status: u16,
        /// What the provider reported.
        detail: String,
    },
    /// The provider did not answer in time.
    Timeout {
        /// Which call ran out of time.
        detail: String,
    },
    /// The call never reached the provider: DNS, TLS, or the connection itself.
    Transport {
        /// What the transport reported.
        detail: String,
    },
}

impl Error {
    /// Whether repeating the identical call could still succeed.
    ///
    /// Everything the provider throttles, fails at, or drops in transit is
    /// worth another attempt; everything that describes the request or the
    /// state of Storage is not, and retrying it only burns quota.
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::RateLimited { .. }
            | Self::ServiceUnavailable { .. }
            | Self::Timeout { .. }
            | Self::Transport { .. }
            | Self::LengthMismatch { .. } => true,
            Self::NotFound { .. }
            | Self::AlreadyExists { .. }
            | Self::PermissionDenied { .. }
            | Self::Unauthenticated { .. }
            | Self::IntegrityMismatch { .. }
            | Self::NotPurged { .. }
            | Self::Unsupported { .. }
            | Self::Rejected { .. }
            | Self::MalformedResponse { .. }
            | Self::Io { .. } => false,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound { object } => write!(f, "no object named {object:?} in Storage"),
            Self::AlreadyExists { object } => {
                write!(f, "an object named {object:?} already exists in Storage")
            }
            Self::PermissionDenied { detail } => write!(f, "Storage refused access: {detail}"),
            Self::Unauthenticated { detail } => {
                write!(f, "Storage rejected the credentials: {detail}")
            }
            Self::IntegrityMismatch { expected, actual } => write!(
                f,
                "Storage stored a digest of {actual}, the bytes sent hash to {expected}"
            ),
            Self::NotPurged { object } => {
                write!(f, "{object:?} is still in Storage after being purged")
            }
            Self::Unsupported { detail } => {
                write!(f, "Storage cannot serve this request: {detail}")
            }
            Self::Rejected { status, detail } => {
                write!(
                    f,
                    "Storage rejected the request with status {status}: {detail}"
                )
            }
            Self::MalformedResponse { detail } => {
                write!(f, "could not read Storage's answer: {detail}")
            }
            Self::LengthMismatch { expected, actual } => {
                write!(f, "expected {expected} bytes, transferred {actual}")
            }
            Self::Io { detail } => write!(f, "local transfer failed: {detail}"),
            Self::RateLimited {
                retry_after: Some(after),
                detail,
            } => write!(
                f,
                "Storage is rate limiting, retry in {}s: {detail}",
                after.as_secs()
            ),
            Self::RateLimited {
                retry_after: None,
                detail,
            } => write!(f, "Storage is rate limiting: {detail}"),
            Self::ServiceUnavailable { status, detail } => {
                write!(f, "Storage failed with status {status}: {detail}")
            }
            Self::Timeout { detail } => write!(f, "Storage did not answer in time: {detail}"),
            Self::Transport { detail } => write!(f, "could not reach Storage: {detail}"),
        }
    }
}

impl error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::Io {
            detail: error.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lost_race_is_not_worth_retrying_unchanged() {
        let error = Error::AlreadyExists {
            object: "jrn-7.cfrt".to_owned(),
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn throttling_and_provider_faults_are_worth_retrying() {
        assert!(Error::RateLimited {
            retry_after: Some(Duration::from_secs(3)),
            detail: "userRateLimitExceeded".to_owned(),
        }
        .is_retryable());
        assert!(Error::ServiceUnavailable {
            status: 503,
            detail: "backendError".to_owned(),
        }
        .is_retryable());
    }
}
