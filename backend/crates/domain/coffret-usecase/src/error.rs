use std::error;
use std::fmt;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use coffret_model::Redacted;

use crate::gateway_failure::GatewayFailure;
use crate::missing::Missing;

/// Result alias for [`ObjectStore`](crate::ObjectStore) operations.
///
/// The crate names two ports, and each fails in its own vocabulary:
/// [`IndexResult`](crate::IndexResult) is the [`Index`](crate::Index)
/// counterpart.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything an [`ObjectStore`](crate::ObjectStore) operation can fail with.
///
/// The variants are the storage vocabulary the use-case layer reasons in, not
/// any one provider's error catalogue: a gateway classifies whatever its SDK or
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
///
/// Every variant that says what a provider or a transport reported carries it
/// in up to two forms. `source` is the value the gateway classified the
/// failure from, handed over whole as a [`GatewayFailure`], so that
/// [`source`](error::Error::source) walks on into the gateway's own chain
/// rather than ending at this port with only a rendering of it. `detail` is a
/// sentence: where a value stands behind it, one that says nothing the value
/// does not — the value's own top line, or the provider's words it holds,
/// redacted as the gateway redacted them — for a caller that reads the field
/// without walking the chain; and the whole of what the gateway said where
/// none does. `Display` renders `detail` only in the second case, since in the
/// first the next link says it, and so a gateway's own account of what it was
/// doing belongs in that value rather than in `detail` alone, where a chain
/// never shows it. Where a gateway composed the sentence itself and had no
/// value behind it, `source` is `None`; that is not a value dropped, there was
/// none.
///
/// The two stay side by side rather than as one field that holds either a
/// composed sentence or a value: a caller that reads `detail` does so without
/// asking which of the two it has, and what it is after — a provider's own
/// message, which the value's line wraps in its status and reason — is a
/// sentence in both.
#[derive(Debug, Clone)]
pub enum Error {
    /// Storage does not hold what the operation asked for.
    ///
    /// Usually one object, and then the name is carried; but a listing, the
    /// configured bucket or folder, and a provider endpoint can all answer this
    /// way too, and none of those has a name a diagnostic may keep. Which of
    /// them it was is [`Missing`](crate::Missing) rather than a sentence in a
    /// name field.
    NotFound {
        /// What the operation asked for and Storage does not hold.
        missing: Missing,
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
        /// The value the gateway classified this from, where it had one.
        ///
        /// `None` where the gateway composed the failure itself and there is
        /// nothing behind it; see [`GatewayFailure`].
        source: Option<GatewayFailure>,
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
        /// The value the gateway classified this from, where it had one.
        ///
        /// `None` where the gateway composed the failure itself and there is
        /// nothing behind it; see [`GatewayFailure`].
        source: Option<GatewayFailure>,
    },
    /// The credentials are missing, expired beyond refresh, or rejected.
    Unauthenticated {
        /// What the provider reported.
        detail: String,
        /// The value the gateway classified this from, where it had one.
        ///
        /// `None` where the gateway composed the failure itself and there is
        /// nothing behind it; see [`GatewayFailure`].
        source: Option<GatewayFailure>,
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
        /// The value the gateway classified this from, where it had one.
        ///
        /// `None` where the gateway composed the failure itself and there is
        /// nothing behind it; see [`GatewayFailure`].
        source: Option<GatewayFailure>,
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
        /// The value the gateway classified this from, where it had one.
        ///
        /// `None` where the gateway composed the failure itself and there is
        /// nothing behind it; see [`GatewayFailure`].
        source: Option<GatewayFailure>,
    },
    /// The provider answered with something this build cannot read.
    MalformedResponse {
        /// What went wrong reading the response.
        detail: String,
        /// The value the gateway classified this from, where it had one.
        ///
        /// `None` where the gateway composed the failure itself and there is
        /// nothing behind it; see [`GatewayFailure`].
        source: Option<GatewayFailure>,
    },
    /// A listing of Storage outran the pages this device reads of one.
    ///
    /// Storage answered every page it was asked for, and what is wrong is the
    /// sequence of them: each one handed back somewhere to carry on and none
    /// said the listing was over, so the walk stopped at its cap rather than
    /// follow it forever. That is not
    /// [`MalformedResponse`](Self::MalformedResponse) — every answer read — and
    /// calling it one would send somebody looking for a response this build
    /// cannot parse.
    ///
    /// Raised by a page loop that walks a listing on the port's behalf: a
    /// gateway that has to page through one of the provider's own listings to
    /// answer a single call, and the commit flow's walk of the Library's
    /// listing, which reports in this vocabulary. The upload step's
    /// `ListingLimitReached`, and the sync's and the freeze's that carry it, are
    /// not replaced by this: that step runs its own page loop over
    /// [`list`](crate::ObjectStore::list) and raises the refusal itself, where
    /// no port call ever failed, so there is nothing this variant could have
    /// said for it.
    ///
    /// Never retryable: a listing that did not end within the cap will not end
    /// within it on the next identical walk either.
    ListingPastCap {
        /// How many pages were read before the walk stopped.
        pages: usize,
        /// The value the gateway classified this from, where it had one.
        ///
        /// `None` where the walk that stopped is this crate's own; see
        /// [`GatewayFailure`].
        source: Option<GatewayFailure>,
    },
    /// A stream stopped short of the count it was held to.
    ///
    /// Two counts hold a drain, and a stream falling short of either lands
    /// here: the length the stream itself declared, which is what a whole read
    /// is judged against, and the length a ranged read asked Storage for, which
    /// is what a ranged read is judged against. Both meet the body in
    /// [`ByteStream::collect_exact`](crate::ByteStream::collect_exact), which
    /// counts what arrived against whichever of the two its caller brought. The
    /// second is the stronger of the two, being the caller's own number rather
    /// than a claim from outside the trust boundary.
    ///
    /// A fetch that streams the object onto disk rather than into memory holds
    /// it to the declared length itself, and raises this the same way when the
    /// transfer ends short of it.
    LengthMismatch {
        /// The count the stream was held to.
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
    ObjectTooLong {
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
        /// The value the gateway classified this from, where it had one.
        ///
        /// `None` where the gateway composed the failure itself and there is
        /// nothing behind it; see [`GatewayFailure`].
        source: Option<GatewayFailure>,
    },
    /// The provider failed on its own side.
    ServiceUnavailable {
        /// The HTTP status the provider answered with.
        status: u16,
        /// What the provider reported.
        detail: String,
        /// The value the gateway classified this from, where it had one.
        ///
        /// `None` where the gateway composed the failure itself and there is
        /// nothing behind it; see [`GatewayFailure`].
        source: Option<GatewayFailure>,
    },
    /// The provider did not answer in time.
    Timeout {
        /// Which call ran out of time.
        detail: String,
        /// The value the gateway classified this from, where it had one.
        ///
        /// `None` where the gateway composed the failure itself and there is
        /// nothing behind it; see [`GatewayFailure`].
        source: Option<GatewayFailure>,
    },
    /// The call never reached the provider: DNS, TLS, or the connection itself.
    Transport {
        /// What the transport reported.
        detail: String,
        /// The value the gateway classified this from, where it had one.
        ///
        /// `None` where the gateway composed the failure itself and there is
        /// nothing behind it; see [`GatewayFailure`].
        source: Option<GatewayFailure>,
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
            Self::ObjectTooLong { .. }
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
            | Self::ListingPastCap { .. }
            | Self::Io { .. }
            | Self::Model { .. } => false,
        }
    }
}

/// Writes `line`, followed by `detail` only where no value stands behind it.
///
/// Every wrapper in this workspace says in `Display` only what its own layer
/// knows and leaves the rest to `source`, so that a chain printed link by link
/// never says one sentence twice. Where the gateway handed its failure over,
/// that value is the next link and says for itself what `detail` would have
/// said, so this line stops at the port's own words for the kind of failure.
/// Where nothing was handed over, `detail` is the whole of what the gateway
/// said, and the line is incomplete without it.
fn with_detail(
    f: &mut fmt::Formatter<'_>,
    line: fmt::Arguments<'_>,
    detail: &str,
    source: &Option<GatewayFailure>,
) -> fmt::Result {
    match source {
        Some(_) => f.write_fmt(line),
        None => write!(f, "{line}: {detail}"),
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A variant with a `detail` says its own line and, where a `source`
        // stands behind it, stops there: see `with_detail`.
        match self {
            Self::NotFound { missing } => write!(f, "{missing}"),
            Self::AlreadyExists { object } => {
                write!(f, "an object named {object:?} already exists in Storage")
            }
            Self::PermissionDenied { detail, source } => {
                with_detail(f, format_args!("Storage refused access"), detail, source)
            }
            Self::LimitReached {
                limit,
                detail,
                source,
            } => with_detail(
                f,
                format_args!("Storage is at its {limit} limit"),
                detail,
                source,
            ),
            Self::Unauthenticated { detail, source } => with_detail(
                f,
                format_args!("Storage rejected the credentials"),
                detail,
                source,
            ),
            Self::IntegrityMismatch { expected, actual } => write!(
                f,
                "Storage stored a digest of {actual}, the bytes sent hash to {expected}"
            ),
            Self::NotPurged { object } => {
                write!(f, "{object:?} is still in Storage after being purged")
            }
            Self::Unsupported { detail, source } => with_detail(
                f,
                format_args!("Storage cannot serve this request"),
                detail,
                source,
            ),
            Self::Rejected {
                status,
                detail,
                source,
            } => with_detail(
                f,
                format_args!("Storage rejected the request with status {status}"),
                detail,
                source,
            ),
            Self::MalformedResponse { detail, source } => with_detail(
                f,
                format_args!("could not read Storage's answer"),
                detail,
                source,
            ),
            Self::ListingPastCap { pages, .. } => write!(
                f,
                "Storage answered, but its listing did not end within the {pages} pages \
                 this device reads of one"
            ),
            Self::LengthMismatch { expected, actual } => {
                write!(f, "expected {expected} bytes, transferred {actual}")
            }
            Self::LengthOverrun { expected } => {
                write!(f, "expected {expected} bytes, and more were transferred")
            }
            Self::ObjectTooLong { declared, ceiling } => write!(
                f,
                "an answer of {declared} bytes was declared, past the {ceiling} \
                 this read takes in"
            ),
            // What the operating system reported is the value `source` hands
            // on, and a caller walking the chain prints it under this line;
            // rendering it here as well would spell one refusal twice over.
            Self::Io { .. } => f.write_str("local transfer failed"),
            Self::RateLimited {
                retry_after: Some(after),
                detail,
                source,
            } => with_detail(
                f,
                format_args!("Storage is rate limiting, retry in {}s", after.as_secs()),
                detail,
                source,
            ),
            Self::RateLimited {
                retry_after: None,
                detail,
                source,
            } => with_detail(f, format_args!("Storage is rate limiting"), detail, source),
            Self::ServiceUnavailable {
                status,
                detail,
                source,
            } => with_detail(
                f,
                format_args!("Storage failed with status {status}"),
                detail,
                source,
            ),
            Self::Timeout { detail, source } => with_detail(
                f,
                format_args!("Storage did not answer in time"),
                detail,
                source,
            ),
            Self::Transport { detail, source } => {
                with_detail(f, format_args!("could not reach Storage"), detail, source)
            }
            // The same, of the domain's own refusal: this says which layer the
            // operation stopped at and leaves the domain's answer to the cause
            // under it.
            Self::Model(_) => {
                f.write_str("a value this operation had to derive is not one the domain admits")
            }
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Io { cause } => Some(cause.as_ref()),
            Self::Model(error) => Some(error),
            // What the gateway classified the failure from, where it handed
            // one over: the chain goes on into the gateway's own links from
            // here.
            Self::PermissionDenied { source, .. }
            | Self::LimitReached { source, .. }
            | Self::Unauthenticated { source, .. }
            | Self::Unsupported { source, .. }
            | Self::Rejected { source, .. }
            | Self::MalformedResponse { source, .. }
            | Self::ListingPastCap { source, .. }
            | Self::RateLimited { source, .. }
            | Self::ServiceUnavailable { source, .. }
            | Self::Timeout { source, .. }
            | Self::Transport { source, .. } => source.as_ref().map(GatewayFailure::as_error),
            // Nothing a Rust error reported: what these carry are facts this
            // layer or a gateway put together out of names, counts and
            // digests. Listed rather than left to a wildcard so that a variant
            // added with a cause has to say here where that cause goes.
            Self::NotFound { .. }
            | Self::AlreadyExists { .. }
            | Self::IntegrityMismatch { .. }
            | Self::NotPurged { .. }
            | Self::LengthMismatch { .. }
            | Self::LengthOverrun { .. }
            | Self::ObjectTooLong { .. } => None,
        }
    }
}

impl Redacted for Error {
    /// What happened, as an identity, wherever a provider or a transport had
    /// something to say about it; the message, where every field of it is an
    /// opaque value.
    ///
    /// A `detail` is never rendered here. It may be whatever the provider or
    /// the transport said, and a provider may echo any part of the request — an
    /// object, a path, the bucket somebody configured — and a transport hangs
    /// the URL it was calling off its own message (spec: EL-5). Holding every
    /// gateway to scrubbing that text before it crosses was a contract nothing
    /// below this line could check, so this rendering no longer relies on it:
    /// those variants render as `Storage::Variant`, with the structured facts
    /// beside them that a log may carry — a status, the name a provider gives
    /// the limit it enforces, how long it asked to be left alone, how many
    /// pages a listing ran to — and nothing else (spec: EL-2). What the
    /// provider actually answered is the gateway's to record, where it read
    /// the answer and can redact it against what it was configured with, and
    /// the value itself travels in `source` for a person reading the chain.
    /// That value is a foreign cause as far as this rendering can tell, so it
    /// stops here rather than going underneath (spec: EL-4).
    ///
    /// The variants rendered as their message are the ones whose every field
    /// is an opaque value — an object name coffret or the provider minted, a
    /// count, a digest — or, for [`NotFound`](Self::NotFound), a
    /// [`Missing`](crate::Missing) whose kinds other than an object are fixed
    /// words. A gateway raising one of them still owes it that the name inside
    /// says only what coffret or the provider minted: never a local path, never
    /// the bucket or the prefix somebody configured, never any name a person
    /// chose. No identity is prepended to those messages: every one of them
    /// says what it is about in so many words, and whatever wraps this has
    /// already said which of its own variants held it.
    ///
    /// Two more are rendered rather than quoted. [`Io`](Self::Io) is this
    /// machine's own failure, and a gateway may have folded a message naming a
    /// local file into the `io::Error` it hands over — the Drive token cache
    /// does exactly that — so what survives is the
    /// [`kind`](io::ErrorKind), which is the half a caller acts on anyway.
    /// [`Model`](Self::Model) is handed to the domain layer's own rendering,
    /// because one of its refusals names a path (spec: EL-1, EL-4).
    fn redacted(&self) -> String {
        match self {
            Self::PermissionDenied { .. } => "Storage::PermissionDenied".to_owned(),
            Self::LimitReached { limit, .. } => format!("Storage::LimitReached(limit={limit})"),
            Self::Unauthenticated { .. } => "Storage::Unauthenticated".to_owned(),
            Self::Unsupported { .. } => "Storage::Unsupported".to_owned(),
            Self::Rejected { status, .. } => format!("Storage::Rejected(status={status})"),
            Self::MalformedResponse { .. } => "Storage::MalformedResponse".to_owned(),
            Self::ListingPastCap { pages, .. } => {
                format!("Storage::ListingPastCap(pages={pages})")
            }
            Self::RateLimited {
                retry_after: Some(after),
                ..
            } => format!("Storage::RateLimited(retry_after={}s)", after.as_secs()),
            Self::RateLimited {
                retry_after: None, ..
            } => "Storage::RateLimited".to_owned(),
            Self::ServiceUnavailable { status, .. } => {
                format!("Storage::ServiceUnavailable(status={status})")
            }
            Self::Timeout { .. } => "Storage::Timeout".to_owned(),
            Self::Transport { .. } => "Storage::Transport".to_owned(),
            Self::NotFound { .. }
            | Self::AlreadyExists { .. }
            | Self::IntegrityMismatch { .. }
            | Self::NotPurged { .. }
            | Self::LengthMismatch { .. }
            | Self::LengthOverrun { .. }
            | Self::ObjectTooLong { .. } => self.to_string(),
            Self::Io { cause } => format!("Io(kind={:?})", cause.kind()),
            Self::Model(error) => error.redacted(),
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
    use coffret_format::Error as FormatError;
    use coffret_model::ControlObjectKind;

    use super::*;

    /// The links a caller printing `{error:#}` reads, outermost first.
    fn chain(error: &dyn error::Error) -> Vec<String> {
        let mut links = vec![error.to_string()];
        let mut below = error.source();
        while let Some(link) = below {
            links.push(link.to_string());
            below = link.source();
        }
        links
    }

    /// The length every refusal below is built around, and the bound it passed.
    ///
    /// Round and small, because what is read off them here is the words beside
    /// them rather than the arithmetic.
    const DECLARED: u64 = 4_097;
    const CEILING: u64 = 4_096;

    // One judgement — a declared length past a bound — crosses the boundary
    // between these two crates on every decode, and is spelled one way on both
    // sides of it. A reader who has just come from `coffret_format` should not
    // have to work out that `TooLarge` and `TooLong`, or `limit` and `ceiling`,
    // are the same thing said twice.
    //
    // A spelling that drifts fails here rather than in review: a renamed field
    // fails to compile, and a renamed variant fails the assertions. What this
    // does not claim is that every variant ending in `TooLong` belongs in the
    // list — `StreamTooLong` deliberately does not, and says why where it is
    // declared.
    #[test]
    fn a_length_past_the_ceiling_is_refused_the_same_way_everywhere() {
        let refusals = [
            Error::ObjectTooLong {
                declared: DECLARED,
                ceiling: CEILING,
            }
            .to_string(),
            FormatError::MetaSectionTooLong {
                declared: DECLARED,
                ceiling: CEILING,
            }
            .to_string(),
            FormatError::ControlObjectTooLong {
                kind: ControlObjectKind::Keyring,
                len: DECLARED,
                ceiling: CEILING,
            }
            .to_string(),
        ];

        for refusal in &refusals {
            assert!(
                refusal.contains(&DECLARED.to_string()),
                "the length that passed the bound is in the sentence: {refusal}",
            );
            assert!(
                refusal.contains(&format!("past the {CEILING}")),
                "and the bound is what it is past, in those words: {refusal}",
            );
        }

        // The variants and their fields, as a caller matching on one reads
        // them. `Debug` is what carries the names out of the types and into
        // something assertable.
        let names = [
            format!(
                "{:?}",
                Error::ObjectTooLong {
                    declared: DECLARED,
                    ceiling: CEILING,
                }
            ),
            format!(
                "{:?}",
                FormatError::MetaSectionTooLong {
                    declared: DECLARED,
                    ceiling: CEILING,
                }
            ),
            format!(
                "{:?}",
                FormatError::ControlObjectTooLong {
                    kind: ControlObjectKind::Keyring,
                    len: DECLARED,
                    ceiling: CEILING,
                }
            ),
        ];

        for name in &names {
            assert!(
                name.contains("TooLong"),
                "a length past a bound is `TooLong` wherever it is raised: {name}",
            );
            assert!(
                name.contains("ceiling"),
                "and the bound it passed is a `ceiling` wherever it is carried: {name}",
            );
        }
    }

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
            source: None,
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
            panic!(
                "an io::Error must arrive as Error::Io, not {}",
                chain(&error).join(": ")
            );
        };
        assert_eq!(cause.kind(), io::ErrorKind::PermissionDenied);
        assert!(error::Error::source(&error).is_some());
        // Nothing about this machine changes while a worker sleeps on it.
        assert!(!error.is_retryable());
    }

    /// Every variant that carries a `detail`, each holding `detail` and
    /// `source`, beside the structured facts a diagnostic event may keep.
    fn every_detail_variant(detail: &str, source: &Option<GatewayFailure>) -> Vec<Error> {
        let detail = detail.to_owned();
        vec![
            Error::PermissionDenied {
                detail: detail.clone(),
                source: source.clone(),
            },
            Error::LimitReached {
                limit: "storageQuotaExceeded".to_owned(),
                detail: detail.clone(),
                source: source.clone(),
            },
            Error::Unauthenticated {
                detail: detail.clone(),
                source: source.clone(),
            },
            Error::Unsupported {
                detail: detail.clone(),
                source: source.clone(),
            },
            Error::Rejected {
                status: 418,
                detail: detail.clone(),
                source: source.clone(),
            },
            Error::MalformedResponse {
                detail: detail.clone(),
                source: source.clone(),
            },
            Error::RateLimited {
                retry_after: Some(Duration::from_secs(3)),
                detail: detail.clone(),
                source: source.clone(),
            },
            Error::RateLimited {
                retry_after: None,
                detail: detail.clone(),
                source: source.clone(),
            },
            Error::ServiceUnavailable {
                status: 503,
                detail: detail.clone(),
                source: source.clone(),
            },
            Error::Timeout {
                detail: detail.clone(),
                source: source.clone(),
            },
            Error::Transport {
                detail,
                source: source.clone(),
            },
        ]
    }

    // Which failure it was, and the facts beside it a log may carry: a status,
    // the name a provider gives its limit, how long it asked to be left alone.
    // Those are what a file of these is grouped by afterwards.
    #[test]
    fn what_storage_answered_is_named_rather_than_quoted() {
        let rendered: Vec<String> = every_detail_variant("backendError", &None)
            .iter()
            .map(Redacted::redacted)
            .collect();

        assert_eq!(
            rendered,
            vec![
                "Storage::PermissionDenied",
                "Storage::LimitReached(limit=storageQuotaExceeded)",
                "Storage::Unauthenticated",
                "Storage::Unsupported",
                "Storage::Rejected(status=418)",
                "Storage::MalformedResponse",
                "Storage::RateLimited(retry_after=3s)",
                "Storage::RateLimited",
                "Storage::ServiceUnavailable(status=503)",
                "Storage::Timeout",
                "Storage::Transport",
            ],
        );
    }

    // A provider may echo any part of the request it refused, and a gateway
    // that forgot to take the configured location back out of its answer used
    // to write that location into the log through this rendering. It no longer
    // can: the text is not rendered at all, so what it names — an object, a
    // folder, the bucket somebody chose — never reaches an event, whether a
    // value stands behind it or not (spec: EL-1, EL-5).
    #[test]
    fn a_detail_that_names_an_object_never_reaches_a_redacted_rendering() {
        let detail = "Access to someones-holiday-photos/albums/spring.jpg was refused \
                      for head-1.cfrt";
        let behind = Some(GatewayFailure::new(io::Error::other(detail)));

        for source in [None, behind] {
            for error in every_detail_variant(detail, &source) {
                let rendered = error.redacted();
                for piece in [
                    detail,
                    "someones-holiday-photos",
                    "albums/spring.jpg",
                    "head-1.cfrt",
                    "refused",
                ] {
                    assert!(
                        !rendered.contains(piece),
                        "{piece:?} reached the redacted rendering {rendered:?}",
                    );
                }
                // The person-facing chain still says it: a refusal is a
                // response, and naming what was refused is part of answering.
                // Where a value stands behind it the value says it, one link
                // down, and the port's own line does not say it again.
                let links = chain(&error);
                assert_eq!(
                    links.iter().filter(|link| link.contains(detail)).count(),
                    1,
                    "{links:?}"
                );
            }
        }
    }

    // The value a gateway handed over is the next link, whole: whoever walks
    // the chain reads what the gateway's own error said, and whoever needs
    // more than words can ask it — here, the kind the operating system gave.
    #[test]
    fn the_value_a_gateway_handed_over_is_where_the_chain_goes_next() {
        let behind = GatewayFailure::new(io::Error::new(
            io::ErrorKind::ConnectionReset,
            "connection reset by peer",
        ));

        for error in every_detail_variant("the call broke off", &Some(behind)) {
            let below = error::Error::source(&error).expect("the value is the next link");
            assert_eq!(below.to_string(), "connection reset by peer");
            let reported = below
                .downcast_ref::<io::Error>()
                .expect("the value crossed as itself, not as a rendering of it");
            assert_eq!(reported.kind(), io::ErrorKind::ConnectionReset);
            assert_eq!(chain(&error).len(), 2, "{:?}", chain(&error));
            assert!(
                !error.to_string().contains("the call broke off"),
                "the value says what went wrong, so the port's line does not: {error}",
            );
        }
    }

    // A sentence a gateway composed out of facts it put together itself has
    // nothing behind it, and the chain says so by ending at the port.
    #[test]
    fn a_failure_the_gateway_composed_itself_ends_the_chain_at_the_port() {
        for error in every_detail_variant("an object name cannot be empty", &None) {
            assert!(error::Error::source(&error).is_none(), "{error}");
        }
    }

    // Storage answered every page and never said the listing was over. That is
    // not an answer this build could not read, and not one a second walk would
    // end differently, so it is reported, and by the number that it ran to.
    #[test]
    fn a_listing_past_the_cap_is_counted_and_not_retried() {
        let error = Error::ListingPastCap {
            pages: 1_000,
            source: None,
        };

        assert!(!error.is_retryable());
        assert!(error.to_string().contains("1000"), "{error}");
        assert_eq!(error.redacted(), "Storage::ListingPastCap(pages=1000)");
        assert!(error::Error::source(&error).is_none());
    }

    // A gateway may fold a message naming one of this device's own files into
    // the `io::Error` it hands over, so the message is not what a diagnostic
    // event renders. It still reaches a person, under this line rather than
    // inside it: this variant says which layer refused, and the value the
    // gateway handed over says the rest.
    #[test]
    fn a_local_failure_is_rendered_as_its_kind_and_not_as_its_message() {
        let error = Error::from(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "could not use the token cache at \"/home/someone/.local/state/coffret/tokens\"",
        ));

        assert_eq!(
            chain(&error),
            vec![
                "local transfer failed".to_owned(),
                "could not use the token cache at \
                 \"/home/someone/.local/state/coffret/tokens\""
                    .to_owned(),
            ],
        );
        assert_eq!(error.redacted(), "Io(kind=PermissionDenied)");
    }

    // The domain is a layer below this port too: the variant says the
    // operation stopped there, and what the domain would not admit is the
    // domain's own sentence.
    #[test]
    fn a_refused_domain_value_reaches_a_caller_as_two_different_sentences() {
        let error = Error::Model(coffret_model::Error::UnnormalizedEntryPath {
            path: "albums/spring.jpg".to_owned(),
        });

        assert_eq!(
            chain(&error),
            vec![
                "a value this operation had to derive is not one the domain admits".to_owned(),
                "the stored path \"albums/spring.jpg\" is not normalized to NFC".to_owned(),
            ],
        );
    }

    #[test]
    fn throttling_and_provider_faults_are_worth_retrying() {
        assert!(Error::RateLimited {
            retry_after: Some(Duration::from_secs(3)),
            detail: "userRateLimitExceeded".to_owned(),
            source: None,
        }
        .is_retryable());
        assert!(Error::ServiceUnavailable {
            status: 503,
            detail: "backendError".to_owned(),
            source: None,
        }
        .is_retryable());
    }
}
