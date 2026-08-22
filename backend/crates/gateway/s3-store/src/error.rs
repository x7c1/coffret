use aws_sdk_s3::error::SdkError;
use aws_smithy_runtime_api::client::orchestrator::HttpResponse;
use aws_smithy_types::error::display::DisplayErrorContext;
use aws_smithy_types::error::metadata::ProvideErrorMetadata;
use coffret_logging::redact;
use coffret_usecase::Error;
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
struct ServiceFailure {
    status: u16,
    code: String,
    body: String,
}

/// Turns an SDK failure into the port's vocabulary.
///
/// The point of doing it here is that nothing above this crate ever sees an S3
/// error code, and nothing above it ever has to read a message to find out what
/// happened: a caller decides what to do from the variant, and whether to try
/// again from [`Error::is_retryable`].
///
/// The operation comes in alongside the object because an answer that falls
/// into a catch-all is recorded as it arrived, and a status on its own says
/// nothing about what was being attempted.
pub fn translate<E>(
    operation: &'static str,
    object: &str,
    error: SdkError<E, HttpResponse>,
) -> Error
where
    E: ProvideErrorMetadata + std::error::Error + 'static,
{
    let detail = describe(&error);
    let Some(failure) = service_failure(&error) else {
        return translate_transport(operation, error, detail);
    };
    match (failure.status, failure.code.as_str()) {
        // S3 asks for a slower pace with `SlowDown`, which it serves under a
        // 503 that would otherwise read as the service being broken.
        (_, "SlowDown") | (429, _) => Error::RateLimited {
            retry_after: None,
            detail,
        },
        (404, _) => {
            // Ordinary: a fresh Library, an interrupted rotation, and a probe
            // all look like this, and none of them is anything to act on.
            debug!(operation, object, "Storage holds no such object");
            Error::NotFound {
                object: object.to_owned(),
            }
        }
        (416, _) => Error::Unsupported { detail },
        (401, _) | (_, "InvalidAccessKeyId") | (_, "SignatureDoesNotMatch") => {
            Error::Unauthenticated { detail }
        }
        (403, _) => {
            failure.record(operation, "Storage refused access");
            Error::PermissionDenied { detail }
        }
        (500..=599, _) => Error::ServiceUnavailable {
            status: failure.status,
            detail,
        },
        // A write with no race to lose, finding the key taken. It is the
        // refusal its status says it is, and it is also a contradiction: an
        // unconditional write carries no condition that could have failed.
        (status, _) if TAKEN.contains(&status) => {
            failure.record(
                operation,
                "a write that carried no condition was refused as though it had",
            );
            Error::Rejected { status, detail }
        }
        _ => {
            failure.record(operation, "Storage rejected the request");
            Error::Rejected {
                status: failure.status,
                detail,
            }
        }
    }
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
pub fn translate_conditional_create<E>(
    operation: &'static str,
    name: &str,
    error: SdkError<E, HttpResponse>,
) -> Error
where
    E: ProvideErrorMetadata + std::error::Error + 'static,
{
    match service_failure(&error) {
        Some(failure) if TAKEN.contains(&failure.status) => Error::AlreadyExists {
            object: name.to_owned(),
        },
        _ => translate(operation, name, error),
    }
}

/// Whether an SDK failure is S3 reporting that nothing is stored there.
pub fn is_not_found<E>(error: &SdkError<E, HttpResponse>) -> bool
where
    E: ProvideErrorMetadata + std::error::Error + 'static,
{
    service_failure(error).is_some_and(|failure| failure.status == 404)
}

impl ServiceFailure {
    /// Keeps what S3 answered, for whoever comes to read the log.
    ///
    /// Only for answers that fall into a catch-all: those are the ones the port
    /// has no state for, so the code above can do nothing but report them, and
    /// what actually came back would otherwise exist nowhere. A key is opaque
    /// and a body of S3's is its own XML, so neither says anything about the
    /// Library.
    fn record(&self, operation: &'static str, what: &str) {
        warn!(
            operation,
            status = self.status,
            reason = %self.code,
            body = %self.body,
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
    E: ProvideErrorMetadata + std::error::Error + 'static,
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
            .map(redact::body)
            .unwrap_or_default(),
    })
}

/// Classifies a failure that never reached S3 or never came back readable.
fn translate_transport<E>(
    operation: &'static str,
    error: SdkError<E, HttpResponse>,
    detail: String,
) -> Error
where
    E: ProvideErrorMetadata + std::error::Error + 'static,
{
    match error {
        SdkError::TimeoutError(_) => Error::Timeout { detail },
        SdkError::DispatchFailure(_) => Error::Transport { detail },
        SdkError::ResponseError(_) => {
            warn!(
                operation,
                detail = %detail,
                "Storage answered with something this build cannot read"
            );
            Error::MalformedResponse { detail }
        }
        // The request was never valid enough to send, which no amount of
        // retrying fixes.
        SdkError::ConstructionFailure(_) => Error::Unsupported { detail },
        // `SdkError` is non-exhaustive: an unknown shape has told us nothing
        // about whether the call landed, so treat it as a transport failure
        // rather than inventing a state for it.
        _ => Error::Transport { detail },
    }
}

/// The failure and everything under it, as one line.
fn describe<E>(error: &SdkError<E, HttpResponse>) -> String
where
    E: ProvideErrorMetadata + std::error::Error + 'static,
{
    DisplayErrorContext(error).to_string()
}
