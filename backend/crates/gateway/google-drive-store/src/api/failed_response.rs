use std::time::Duration;

use coffret_usecase::Error;
use serde::Deserialize;

use crate::http::HttpResponse;

/// Drive's error envelope.
#[derive(Debug, Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

/// What the envelope carries.
#[derive(Debug, Deserialize)]
struct ErrorBody {
    message: Option<String>,
    errors: Option<Vec<ErrorDetail>>,
}

/// One reason inside the envelope.
#[derive(Debug, Deserialize)]
struct ErrorDetail {
    reason: Option<String>,
}

/// The reasons Drive gives for a 403 that mean "later, not never".
///
/// A 403 is otherwise a refusal, and telling the two apart is the difference
/// between a caller backing off and a caller giving up on a Library it is
/// perfectly entitled to. The reason is a field of Drive's error document, not
/// prose in its message, so reading it is reading structured data.
const THROTTLING_REASONS: [&str; 4] = [
    "rateLimitExceeded",
    "userRateLimitExceeded",
    "sharingRateLimitExceeded",
    "dailyLimitExceeded",
];

/// The reason Drive gives when a create names an identifier already in use.
const DUPLICATE_REASON: &str = "duplicate";

/// A response Drive refused with, read into the parts that decide what it means.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedResponse {
    status: u16,
    reason: String,
    detail: String,
    retry_after: Option<Duration>,
}

impl FailedResponse {
    /// Reads a non-2xx response.
    ///
    /// The body is consumed here: the caller has already decided the call
    /// failed, and what is left to learn is only why.
    pub async fn read(response: HttpResponse) -> Self {
        let status = response.status();
        let retry_after = response
            .header("retry-after")
            .and_then(|value| value.parse().ok())
            .map(Duration::from_secs);

        let body = response
            .into_body()
            .into_bytes()
            .await
            .unwrap_or_else(|error| error.to_string().into_bytes());

        let envelope: Option<ErrorEnvelope> = serde_json::from_slice(&body).ok();
        let reason = envelope
            .as_ref()
            .and_then(|envelope| envelope.error.errors.as_ref())
            .and_then(|errors| errors.first())
            .and_then(|first| first.reason.clone())
            .unwrap_or_default();

        let detail = envelope
            .as_ref()
            .and_then(|envelope| envelope.error.message.clone())
            .unwrap_or_else(|| String::from_utf8_lossy(&body).into_owned());

        Self {
            status,
            reason,
            detail,
            retry_after,
        }
    }

    /// What the failure means to the port.
    pub fn into_error(self, object: &str) -> Error {
        let Self {
            status,
            reason,
            detail,
            retry_after,
        } = self;

        match status {
            401 => Error::Unauthenticated { detail },
            403 if THROTTLING_REASONS.contains(&reason.as_str()) => Error::RateLimited {
                retry_after,
                detail,
            },
            403 => Error::PermissionDenied { detail },
            404 => Error::NotFound {
                object: object.to_owned(),
            },
            416 => Error::Unsupported { detail },
            429 => Error::RateLimited {
                retry_after,
                detail,
            },
            500..=599 => Error::ServiceUnavailable { status, detail },
            _ => Error::Rejected { status, detail },
        }
    }

    /// What the failure of a conditional create means to the port.
    ///
    /// A create that names an already-used identifier is a lost commit race,
    /// not a fault: the caller refreshes the control head and retries rather
    /// than concluding it cannot write. Drive reports it as a conflict, or as a
    /// 4xx whose reason is `duplicate`.
    pub fn into_conditional_create_error(self, name: &str) -> Error {
        if self.status == 409 || self.reason == DUPLICATE_REASON {
            return Error::AlreadyExists {
                object: name.to_owned(),
            };
        }
        self.into_error(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coffret_usecase::ByteStream;

    /// A response Drive would have sent.
    fn refusal(status: u16, body: &str) -> HttpResponse {
        HttpResponse::new(status, Vec::new(), ByteStream::from(body.as_bytes()))
    }

    /// Drive's envelope for one reason.
    fn envelope(reason: &str, message: &str) -> String {
        format!(r#"{{"error":{{"message":"{message}","errors":[{{"reason":"{reason}"}}]}}}}"#)
    }

    #[tokio::test]
    async fn throttling_dressed_as_a_refusal_is_read_as_throttling() {
        let response = refusal(
            403,
            &envelope("userRateLimitExceeded", "User rate limit exceeded."),
        );
        let error = FailedResponse::read(response)
            .await
            .into_error("jrn-1.cfrt");

        assert!(matches!(error, Error::RateLimited { .. }));
        assert!(error.is_retryable());
    }

    #[tokio::test]
    async fn a_genuine_refusal_stays_a_refusal() {
        let response = refusal(403, &envelope("insufficientFilePermissions", "No access."));
        let error = FailedResponse::read(response)
            .await
            .into_error("jrn-1.cfrt");

        assert!(matches!(error, Error::PermissionDenied { .. }));
        assert!(!error.is_retryable());
    }

    #[tokio::test]
    async fn a_retry_after_header_is_carried_through() {
        let response = HttpResponse::new(
            429,
            vec![("retry-after".to_owned(), "17".to_owned())],
            ByteStream::from(envelope("rateLimitExceeded", "Slow down.").as_bytes()),
        );
        let error = FailedResponse::read(response)
            .await
            .into_error("jrn-1.cfrt");

        match &error {
            // The header is what tells the caller how long to wait, so the
            // parsed duration matters as much as the variant.
            Error::RateLimited {
                retry_after,
                detail,
            } => {
                assert_eq!(*retry_after, Some(Duration::from_secs(17)));
                assert_eq!(detail, "Slow down.");
            }
            other => panic!("expected throttling with a retry-after, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_duplicate_identifier_is_a_lost_race_and_not_a_fault() {
        let response = refusal(400, &envelope("duplicate", "A file with that id exists."));
        let error = FailedResponse::read(response)
            .await
            .into_conditional_create_error("jrn-1.cfrt");

        match &error {
            Error::AlreadyExists { object } => assert_eq!(object, "jrn-1.cfrt"),
            other => panic!("expected a lost conditional create, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn a_body_that_is_not_drives_envelope_still_classifies_by_status() {
        let response = refusal(503, "<html>backend error</html>");
        let error = FailedResponse::read(response)
            .await
            .into_error("jrn-1.cfrt");

        assert!(matches!(
            error,
            Error::ServiceUnavailable { status: 503, .. }
        ));
        assert!(error.is_retryable());
    }
}
