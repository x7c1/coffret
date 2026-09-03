use std::error;
use std::fmt;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use coffret_model::Redacted;

/// Result alias for [`ObjectStore`](crate::ObjectStore) operations.
///
/// The crate names two ports, and each fails in its own vocabulary:
/// [`IndexResult`](crate::IndexResult) is the [`Index`](crate::Index)
/// counterpart.
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
    /// A limit the provider enforces has been reached, and nothing about the
    /// request is wrong.
    ///
    /// The distinction from [`Error::PermissionDenied`] is what a person is
    /// told to go and look at. A provider often answers both the same way —
    /// Drive reports a full account and a missing permission alike as a 403 —
    /// and reporting "Storage refused access" for a Drive that is simply full
    /// sends somebody to inspect an OAuth grant that was never the problem.
    ///
    /// Never retryable: a limit that is reached stays reached until the account
    /// or the Library changes, and asking again only spends quota finding that
    /// out.
    LimitReached {
        /// Which limit the provider says was reached, as the provider names it.
        limit: String,
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
    /// by a different store, an object name it has no way to represent, or a
    /// body larger than it can send.
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
    /// A stream carried fewer bytes than its declared length.
    LengthMismatch {
        /// The length the stream declared.
        expected: u64,
        /// The length actually transferred.
        actual: u64,
    },
    /// A stream carried more bytes than its declared length.
    ///
    /// Separate from [`LengthMismatch`](Self::LengthMismatch) because the two
    /// are known differently. A short answer is known exactly — it ended, and
    /// the count is what arrived. A long one is only known to be long: the read
    /// stops one byte past what was asked for rather than growing to whatever a
    /// provider decided to send, so how much more there was is not something
    /// this device paid to find out.
    LengthOverrun {
        /// The length the answer was held to, and passed.
        expected: u64,
    },
    /// An answer declares more bytes than what may legitimately be there.
    ///
    /// Raised before any of it is read, against the ceiling the reader brought
    /// for the thing it asked for. Where that is a Storage Object read whole —
    /// a control object — the ceiling is the format's, because a control
    /// object's size is what its payload schema can account for; where it is one
    /// of the provider's own documents, the ceiling is the gateway's, because no
    /// schema of this Library's says anything about those. Either way an account
    /// somebody else has written into, or a provider answering for one, cannot
    /// spend this device's memory by claiming a size. Nothing about the claim
    /// has been authenticated, and nothing was allocated for it.
    ///
    /// Never retryable: the claim is what it is, and asking again gets the same
    /// answer.
    ObjectTooLarge {
        /// The length the answer declared.
        declared: u64,
        /// The most this read was willing to take in.
        ceiling: u64,
    },
    /// Reading or writing the local end of a transfer failed.
    ///
    /// The cause travels as the value the operating system produced rather than
    /// as its message: its [`kind`](io::Error::kind) is what separates a full
    /// disk from a path that is gone, and stringifying it on the way in would
    /// leave a caller matching on prose to tell them apart. It is shared behind
    /// an [`Arc`] because this error is [`Clone`] — a report and a retry may
    /// each hold one — and [`io::Error`] is not.
    Io {
        /// What the operating system reported.
        cause: Arc<io::Error>,
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
    /// A value the operation had to derive is not one the domain admits — the
    /// last representable generation has no successor to commit into, for
    /// instance.
    ///
    /// Storage was never asked anything, so this says nothing about it.
    Model(coffret_model::Error),
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
            | Self::LengthMismatch { .. }
            | Self::LengthOverrun { .. } => true,
            // A size Storage states about an object it holds is not a transfer
            // that went wrong, and a second identical read is answered with the
            // same number.
            Self::ObjectTooLarge { .. }
            | Self::NotFound { .. }
            | Self::AlreadyExists { .. }
            | Self::PermissionDenied { .. }
            | Self::LimitReached { .. }
            | Self::Unauthenticated { .. }
            | Self::IntegrityMismatch { .. }
            | Self::NotPurged { .. }
            | Self::Unsupported { .. }
            | Self::Rejected { .. }
            | Self::MalformedResponse { .. }
            | Self::Io { .. }
            | Self::Model { .. } => false,
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
            Self::LimitReached { limit, detail } => {
                write!(f, "Storage is at its {limit} limit: {detail}")
            }
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
            Self::LengthOverrun { expected } => {
                write!(f, "expected {expected} bytes, and more were transferred")
            }
            Self::ObjectTooLarge { declared, ceiling } => write!(
                f,
                "an answer of {declared} bytes was declared, past the {ceiling} \
                 this read takes in"
            ),
            Self::Io { cause } => write!(f, "local transfer failed: {cause}"),
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
            Self::Model(error) => write!(f, "{error}"),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Io { cause } => Some(cause.as_ref()),
            Self::Model(error) => Some(error),
            // Nothing a Rust error reported: what these carry is what a
            // provider said, or facts this layer put together itself. Listed
            // rather than left to a wildcard so that a variant added with a
            // cause has to say here where that cause goes.
            Self::NotFound { .. }
            | Self::AlreadyExists { .. }
            | Self::PermissionDenied { .. }
            | Self::LimitReached { .. }
            | Self::Unauthenticated { .. }
            | Self::IntegrityMismatch { .. }
            | Self::NotPurged { .. }
            | Self::Unsupported { .. }
            | Self::Rejected { .. }
            | Self::MalformedResponse { .. }
            | Self::LengthMismatch { .. }
            | Self::LengthOverrun { .. }
            | Self::ObjectTooLarge { .. }
            | Self::RateLimited { .. }
            | Self::ServiceUnavailable { .. }
            | Self::Timeout { .. }
            | Self::Transport { .. } => None,
        }
    }
}

impl Redacted for Error {
    /// The message, for everything that is genuinely about Storage.
    ///
    /// This vocabulary is what a log file exists to record: what a provider
    /// actually answered, in a form a person can read afterwards and decide
    /// from. Keeping it is the whole point, and it is the one rendering in the
    /// workspace where a message survives — which makes it the one place the
    /// rule is a contract on whoever builds the value rather than a property
    /// the rendering holds by itself. A gateway raising one of these owes it
    /// that `object`, `limit` and `detail` say only what the provider stated
    /// or what the gateway composed out of opaque values: never a local path,
    /// never the bucket or the prefix somebody configured, never any name a
    /// person chose. Nothing below this line checks that, and a gateway that
    /// folds its own account of a local file into one of these fields writes
    /// that file into the log.
    ///
    /// Two variants are rendered rather than quoted. [`Io`](Self::Io) is this
    /// machine's own failure, and a gateway may have folded a message naming a
    /// local file into the `io::Error` it hands over — the Drive token cache
    /// does exactly that — so what survives is the
    /// [`kind`](io::ErrorKind), which is the half a caller acts on anyway. It
    /// is also where a local failure crossing this port belongs, for exactly
    /// that reason: classified as anything else, its message is kept.
    /// [`Model`](Self::Model) is handed to the domain layer's own rendering,
    /// because one of its refusals names a path (spec: EP-1).
    ///
    /// No identity is prepended to the kept messages: every one of them opens
    /// with the word `Storage`, and whatever wraps this has already said which
    /// of its own variants held it.
    fn redacted(&self) -> String {
        match self {
            Self::Io { cause } => format!("Io(kind={:?})", cause.kind()),
            Self::Model(error) => error.redacted(),
            other => other.to_string(),
        }
    }
}

impl From<coffret_model::Error> for Error {
    fn from(error: coffret_model::Error) -> Self {
        Self::Model(error)
    }
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::Io {
            cause: Arc::new(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lost_race_is_not_worth_retrying_unchanged() {
        let error = Error::AlreadyExists {
            object: "head-7.cfrt".to_owned(),
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn a_limit_that_has_been_reached_is_not_waited_out() {
        // Throttling passes; a limit does not, however alike the two look
        // coming off the wire. Nothing about a full account changes while a
        // worker sleeps on it.
        let error = Error::LimitReached {
            limit: "storageQuotaExceeded".to_owned(),
            detail: "The user's Drive storage quota has been exceeded.".to_owned(),
        };
        assert!(!error.is_retryable());
    }

    #[test]
    fn a_local_failure_carries_the_operating_system_error_it_saw() {
        let error = Error::from(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "the spool directory is not writable",
        ));

        let Error::Io { cause } = &error else {
            panic!("an io::Error must arrive as Error::Io, not {error}");
        };
        assert_eq!(cause.kind(), io::ErrorKind::PermissionDenied);
        assert!(error::Error::source(&error).is_some());
        // Nothing about this machine changes while a worker sleeps on it.
        assert!(!error.is_retryable());
    }

    // What a provider said is what the file is kept for, so a redacted
    // rendering keeps it: the `detail` reached this variant already redacted by
    // the gateway that read the body.
    #[test]
    fn what_storage_answered_survives_redaction() {
        let error = Error::ServiceUnavailable {
            status: 503,
            detail: "backendError".to_owned(),
        };

        assert_eq!(
            error.redacted(),
            "Storage failed with status 503: backendError",
        );
    }

    // A gateway may fold a message naming one of this device's own files into
    // the `io::Error` it hands over, so the message is not what a log line
    // renders.
    #[test]
    fn a_local_failure_is_rendered_as_its_kind_and_not_as_its_message() {
        let error = Error::from(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "could not use the token cache at \"/home/someone/.local/state/coffret/tokens\"",
        ));

        assert!(error.to_string().contains("/home/someone"));
        assert_eq!(error.redacted(), "Io(kind=PermissionDenied)");
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
