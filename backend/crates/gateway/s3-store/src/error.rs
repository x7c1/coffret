use aws_sdk_s3::error::SdkError;
use aws_smithy_runtime_api::client::orchestrator::HttpResponse;
use aws_smithy_types::error::display::DisplayErrorContext;
use aws_smithy_types::error::metadata::ProvideErrorMetadata;
use coffret_usecase::Error;

/// What S3 answered, reduced to the two things the port cares about.
struct ServiceFailure {
    status: u16,
    code: String,
}

/// Turns an SDK failure into the port's vocabulary.
///
/// The point of doing it here is that nothing above this crate ever sees an S3
/// error code, and nothing above it ever has to read a message to find out what
/// happened: a caller decides what to do from the variant, and whether to try
/// again from [`Error::is_retryable`].
pub fn translate<E>(object: &str, error: SdkError<E, HttpResponse>) -> Error
where
    E: ProvideErrorMetadata + std::error::Error + 'static,
{
    let detail = describe(&error);
    let Some(failure) = service_failure(&error) else {
        return translate_transport(error, detail);
    };
    match (failure.status, failure.code.as_str()) {
        // S3 asks for a slower pace with `SlowDown`, which it serves under a
        // 503 that would otherwise read as the service being broken.
        (_, "SlowDown") | (429, _) => Error::RateLimited {
            retry_after: None,
            detail,
        },
        (404, _) => Error::NotFound {
            object: object.to_owned(),
        },
        (416, _) => Error::Unsupported { detail },
        (401, _) | (_, "InvalidAccessKeyId") | (_, "SignatureDoesNotMatch") => {
            Error::Unauthenticated { detail }
        }
        (403, _) => Error::PermissionDenied { detail },
        (500..=599, _) => Error::ServiceUnavailable {
            status: failure.status,
            detail,
        },
        _ => Error::Rejected {
            status: failure.status,
            detail,
        },
    }
}

/// Turns the failure of a conditional create into the port's vocabulary.
///
/// A conditional PUT that finds the key taken is the one case where a 4xx is a
/// state rather than a fault: it means another writer consumed the commit slot
/// first, so the caller refreshes the control head and retries rather than
/// treating the object as unwritable. S3 answers `PreconditionFailed` when the
/// key was already there and `ConditionalRequestConflict` when a concurrent
/// write got in during this one, and both mean the same thing here.
pub fn translate_conditional_create<E>(name: &str, error: SdkError<E, HttpResponse>) -> Error
where
    E: ProvideErrorMetadata + std::error::Error + 'static,
{
    match service_failure(&error) {
        Some(failure) if matches!(failure.status, 409 | 412) => Error::AlreadyExists {
            object: name.to_owned(),
        },
        _ => translate(name, error),
    }
}

/// Whether an SDK failure is S3 reporting that nothing is stored there.
pub fn is_not_found<E>(error: &SdkError<E, HttpResponse>) -> bool
where
    E: ProvideErrorMetadata + std::error::Error + 'static,
{
    service_failure(error).is_some_and(|failure| failure.status == 404)
}

/// The status and error code of a failure S3 itself answered with.
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
    })
}

/// Classifies a failure that never reached S3 or never came back readable.
fn translate_transport<E>(error: SdkError<E, HttpResponse>, detail: String) -> Error
where
    E: ProvideErrorMetadata + std::error::Error + 'static,
{
    match error {
        SdkError::TimeoutError(_) => Error::Timeout { detail },
        SdkError::DispatchFailure(_) => Error::Transport { detail },
        SdkError::ResponseError(_) => Error::MalformedResponse { detail },
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
