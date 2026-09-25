use aws_sdk_s3::error::SdkError;
use aws_smithy_runtime_api::client::orchestrator::HttpResponse;
use aws_smithy_types::error::display::DisplayErrorContext;
use aws_smithy_types::error::metadata::ProvideErrorMetadata;
use coffret_logging::redact::{self, PrivateValues};
use coffret_usecase::{Error, GatewayFailure, Missing};
use tracing::{debug, warn};

/// The statuses S3 answers a conditional create whose key is taken with.
///
/// `PreconditionFailed` (412) when the key was already there, and
/// `ConditionalRequestConflict` (409) when a concurrent conditional write got
/// in during this one.
///
/// Strictly, 409 says less than 412 does: it says another conditional operation
/// was in flight, not that the object exists, so if that operation then failed
/// the key is still free and this caller was told it lost a race nobody won.
/// Folding the two together is safe only because losing is never the end of the
/// story — every loser reads the slot back to see what took it (spec: CP-4,
/// CP-5, CK-11), and a slot that turns out to be empty sends it round the
/// commit again rather than leaving it stopped on a refusal (spec: CP-3).
const TAKEN: [u16; 2] = [409, 412];

/// What S3 answered, reduced to what decides the meaning and what records it.
///
/// The code and the body are held as they arrived. The code is what the match
/// in [`classify`] branches on, so it never passes through a log rendering
/// first: redaction is there to decide what an event may say, not what a
/// failure means. [`Self::record`] renders the two where they are recorded
/// instead.
struct ServiceFailure {
    status: u16,
    code: String,
    body: Vec<u8>,
}

/// Turns an SDK failure into the port's vocabulary.
///
/// The point of doing it here is that nothing above this crate ever sees an S3
/// error code, and nothing above it ever has to read a message to find out what
/// happened: a caller decides what to do from the variant, and whether to try
/// again from [`Error::is_retryable`].
///
/// The operation comes in alongside the subject because every refusal but a
/// missing key is recorded here, and a status on its own says nothing
/// about what was being attempted. `missing` is what the call asked for: a
/// call that addresses one object hands over the name coffret minted, and a
/// caller that addresses no one object — [`classify_listing`], or the
/// pre-store bucket check — hands over the kind of thing it asked about
/// instead. It is what the `object` field of the not-found event is built
/// from, through [`Missing::subject`], so that field says one word per kind
/// rather than quoting the location a Library was configured into.
///
/// `private` is what the caller was configured with — for a store, the bucket
/// and the prefix; for the pre-store bucket check, the bucket alone, there
/// being no Library prefix to ask about yet — and it comes in on every call
/// rather than only on the ones that address a location: an object identifier
/// is opaque, but S3 answers about it by quoting the whole of what it was asked
/// for, bucket and key together (spec: EL-5). There is no default for it here,
/// because this table does not know better than its caller what was private
/// about the call — so provider text cannot be recorded without somebody having
/// answered that question.
pub fn classify<E>(
    operation: &'static str,
    missing: Missing,
    error: SdkError<E, HttpResponse>,
    private: &PrivateValues,
) -> Error
where
    E: ProvideErrorMetadata + std::error::Error + Send + Sync + 'static,
{
    let detail = redact::text_without(&describe(&error), private);
    let Some(failure) = service_failure(&error) else {
        return classify_transport(operation, error, detail);
    };
    // The SDK's failure itself, whole, for whoever walks the chain past the
    // port: the operation's own error with the code and message S3 gave, and
    // the raw response under it. `detail` is its rendering with the private
    // values taken out, for a caller that reads the field without walking.
    //
    // Handed over as the SDK raised it, not wrapped in a type of this crate's
    // whose `Display` is redacted, and the same holds for every failure
    // `classify_transport` hands over. The chain is read by the person who
    // invoked the operation, and a bucket or a prefix S3 echoes in it is that
    // person's own configuration, which a response may name (spec: EL-1). A
    // diagnostic event never walks it — the port's rendering for one stops at
    // the port (spec: EL-2, EL-4) — and S3's words reach a log only through
    // `ServiceFailure::record`, redacted. A wrapper that kept those words out
    // of the chain would have to keep the SDK's error out of it too, and that
    // error's code, status and raw response are what handing it over is for.
    let source = Some(GatewayFailure::new(error));
    match (failure.status, failure.code.as_str()) {
        // S3 asks for a slower pace with `SlowDown`, which it serves under a
        // 503 that would otherwise read as the service being broken.
        (_, "SlowDown") | (429, _) => {
            failure.record(operation, "Storage is rate limiting", private);
            Error::RateLimited {
                retry_after: None,
                detail,
                source,
            }
        }
        (404, _) => {
            // Ordinary: a fresh Library, an interrupted rotation, and a probe
            // all look like this, and none of them is anything to act on.
            debug!(
                operation,
                object = missing.subject(),
                "Storage holds no such object"
            );
            Error::NotFound { missing }
        }
        (416, _) => {
            failure.record(operation, "Storage cannot serve this request", private);
            Error::Unsupported { detail, source }
        }
        (401, _) | (_, "InvalidAccessKeyId") | (_, "SignatureDoesNotMatch") => {
            failure.record(operation, "Storage rejected the credentials", private);
            Error::Unauthenticated { detail, source }
        }
        (403, _) => {
            failure.record(operation, "Storage refused access", private);
            Error::PermissionDenied { detail, source }
        }
        (500..=599, _) => {
            failure.record(operation, "Storage failed on its own side", private);
            Error::ServiceUnavailable {
                status: failure.status,
                detail,
                source,
            }
        }
        // A write with no race to lose, finding the key taken. It is the
        // refusal its status says it is, and it is also a contradiction: an
        // unconditional write carries no condition that could have failed.
        (status, _) if TAKEN.contains(&status) => {
            failure.record(
                operation,
                "a write that carried no condition was refused as though it had",
                private,
            );
            Error::Rejected {
                status,
                detail,
                source,
            }
        }
        _ => {
            failure.record(operation, "Storage rejected the request", private);
            Error::Rejected {
                status: failure.status,
                detail,
                source,
            }
        }
    }
}

/// Turns the failure of a call that addresses one object into the port's
/// vocabulary.
pub fn classify_object<E>(
    operation: &'static str,
    name: &str,
    error: SdkError<E, HttpResponse>,
    private: &PrivateValues,
) -> Error
where
    E: ProvideErrorMetadata + std::error::Error + Send + Sync + 'static,
{
    classify(operation, Missing::Object(name.to_owned()), error, private)
}

/// Turns a failed listing into the port's vocabulary without retaining the
/// configured location that the provider may echo.
///
/// A listing addresses a private location rather than one opaque object, so
/// what it reports missing is its kind: there is no name to report, and the
/// prefix is what would otherwise fill the field.
pub fn classify_listing<E>(error: SdkError<E, HttpResponse>, private: &PrivateValues) -> Error
where
    E: ProvideErrorMetadata + std::error::Error + Send + Sync + 'static,
{
    classify("list", Missing::Listing, error, private)
}

/// Turns the failure of a conditional create into the port's vocabulary.
///
/// A conditional PUT that finds the key taken is the one case where a 4xx is a
/// state rather than a fault: it means another writer consumed the commit slot
/// first, so the caller refreshes the control head and retries rather than
/// treating the object as unwritable. S3 answers `PreconditionFailed` when the
/// key was already there and `ConditionalRequestConflict` when a concurrent
/// write got in during this one, and both mean the same thing here. Nothing is
/// recorded for it — losing a race is the commit protocol working.
pub fn classify_conditional_create<E>(
    operation: &'static str,
    name: &str,
    error: SdkError<E, HttpResponse>,
    private: &PrivateValues,
) -> Error
where
    E: ProvideErrorMetadata + std::error::Error + Send + Sync + 'static,
{
    match service_failure(&error) {
        Some(failure) if TAKEN.contains(&failure.status) => Error::AlreadyExists {
            object: name.to_owned(),
        },
        // A refusal for any other reason is one nobody has a state for, so it
        // goes through the same table — and carries the same private values,
        // because a conditional PUT quotes the location an ordinary one does.
        _ => classify_object(operation, name, error, private),
    }
}

/// Whether an SDK failure is S3 reporting that nothing is stored there.
pub fn is_not_found<E>(error: &SdkError<E, HttpResponse>) -> bool
where
    E: ProvideErrorMetadata + std::error::Error + Send + Sync + 'static,
{
    service_failure(error).is_some_and(|failure| failure.status == 404)
}

impl ServiceFailure {
    /// Keeps what S3 answered, for whoever comes to read the log.
    ///
    /// For every refusal but a missing key, which is ordinary, and a lost
    /// conditional create, which is the commit protocol working. This is the
    /// one place S3's words reach a log: the port error carries them to a
    /// person, and its rendering for an event names the refusal and its
    /// structured facts without them (spec: EL-2). Provider text is recorded
    /// only after credentials and every private request location have been
    /// removed.
    fn record(&self, operation: &'static str, what: &str, private: &PrivateValues) {
        warn!(
            operation,
            status = self.status,
            reason = %redact::text_without(&self.code, private),
            body = %redact::body_without(&self.body, private),
            "{what}",
        );
    }
}

/// The status, error code, and body of a failure S3 itself answered with.
///
/// `None` means the call never became an answer — it timed out, failed to
/// dispatch, or came back unreadable.
fn service_failure<E>(error: &SdkError<E, HttpResponse>) -> Option<ServiceFailure>
where
    E: ProvideErrorMetadata + std::error::Error + Send + Sync + 'static,
{
    let SdkError::ServiceError(service) = error else {
        return None;
    };
    Some(ServiceFailure {
        status: service.raw().status().as_u16(),
        code: service.err().code().unwrap_or_default().to_owned(),
        // A refusal's body is held in memory by the time it is a failure. Where
        // it is not — a streaming body the SDK did not buffer — there is
        // nothing to record and the code and status carry the event alone.
        body: service
            .raw()
            .body()
            .bytes()
            .map(<[u8]>::to_vec)
            .unwrap_or_default(),
    })
}

/// Classifies a failure that never reached S3 or never came back readable.
fn classify_transport<E>(
    operation: &'static str,
    error: SdkError<E, HttpResponse>,
    detail: String,
) -> Error
where
    E: ProvideErrorMetadata + std::error::Error + Send + Sync + 'static,
{
    // Matched in place, so that the arms bind nothing and the failure is still
    // whole to hand over as the value behind whichever variant it becomes.
    match error {
        SdkError::TimeoutError(_) => Error::Timeout {
            detail,
            source: Some(GatewayFailure::new(error)),
        },
        SdkError::DispatchFailure(_) => Error::Transport {
            detail,
            source: Some(GatewayFailure::new(error)),
        },
        SdkError::ResponseError(_) => {
            warn!(
                operation,
                detail = %detail,
                "Storage answered with something this build cannot read"
            );
            Error::MalformedResponse {
                detail,
                source: Some(GatewayFailure::new(error)),
            }
        }
        // The request was never valid enough to send, which no amount of
        // retrying fixes.
        SdkError::ConstructionFailure(_) => Error::Unsupported {
            detail,
            source: Some(GatewayFailure::new(error)),
        },
        // `SdkError` is non-exhaustive: an unknown shape has told us nothing
        // about whether the call landed, so treat it as a transport failure
        // rather than inventing a state for it.
        _ => Error::Transport {
            detail,
            source: Some(GatewayFailure::new(error)),
        },
    }
}

/// The failure and everything under it, as one line.
fn describe<E>(error: &SdkError<E, HttpResponse>) -> String
where
    E: ProvideErrorMetadata + std::error::Error + Send + Sync + 'static,
{
    DisplayErrorContext(error).to_string()
}
