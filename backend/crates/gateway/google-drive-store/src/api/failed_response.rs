use std::time::Duration;

use coffret_logging::redact::{self, PrivateValues};
use coffret_usecase::{Error, Missing};
use serde::Deserialize;
use tracing::{debug, warn};

use crate::answer_ceiling::MAX_DOCUMENT_LEN;
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

/// The reasons Drive gives for a 403 that mean a limit has been reached.
///
/// None of these is a permission problem, and none is throttling either: the
/// Drive is full, or the account has as many items as Drive will hold (500
/// million), or a folder has as many children as one folder may have (500,000).
/// They are matched by name and become [`Error::LimitReached`], which carries
/// what turns on the distinction; a 403 naming anything else is still a refusal
/// of access.
const LIMIT_REASONS: [&str; 3] = [
    "storageQuotaExceeded",
    "activeItemCreationLimitExceeded",
    "numChildrenInNonRootLimitExceeded",
];

/// The reason Drive gives when a create names an identifier already in use.
const DUPLICATE_REASON: &str = "duplicate";

/// The status Drive answers a create whose identifier is already taken with.
const CONFLICT: u16 = 409;

/// A response Drive refused with, read into the parts that decide what it means.
///
/// Not comparable by equality: what is asked of a refusal is what it means —
/// which of the port's errors it becomes — and holding a refusal to that
/// leaves it free to grow a field for more of what Drive said.
#[derive(Debug, Clone)]
pub struct FailedResponse {
    operation: &'static str,
    status: u16,
    reason: String,
    detail: String,
    body: String,
    retry_after: Option<Duration>,
}

impl FailedResponse {
    /// Reads a non-2xx response.
    ///
    /// The body is consumed here: the caller has already decided the call
    /// failed, and what is left to learn is only why. The operation comes in
    /// with it because a status and a reason say nothing on their own without
    /// what was being attempted, and whoever reads the log afterwards was not
    /// there to see.
    ///
    /// Only a document's worth of it is taken in, and taken off the front rather
    /// than held against a declared length. A refusal is read for what it
    /// explains, and what explains it is Drive's error envelope at the start of
    /// the body; an answer longer than that explains nothing more, and the one
    /// case where it could be arbitrarily long — a refusal of a `get`, whose
    /// answers otherwise carry a Storage Object — is exactly where believing it
    /// would cost the most.
    ///
    /// `private` is what the caller was configured with and Drive may echo. A
    /// call that addresses one file Drive minted the id of names nothing, with
    /// [`PrivateValues::none`]: an id a provider minted is permitted evidence
    /// rather than somebody's own arrangement, and saying so at the call site is
    /// what keeps the question from going unasked (spec: EL-5). What a
    /// refusal is *reported* about stays the caller's own to choose — it is
    /// passed to [`Self::into_error`], not read out of the answer.
    pub async fn read(
        response: HttpResponse,
        operation: &'static str,
        private: &PrivateValues,
    ) -> Self {
        let status = response.status();
        let retry_after = response
            .header("retry-after")
            .and_then(|value| value.parse().ok())
            .map(Duration::from_secs);

        let bytes = response
            .into_body()
            .collect_front(MAX_DOCUMENT_LEN)
            .await
            .unwrap_or_else(|error| error.to_string().into_bytes());

        // Kept as it arrived, short of anything that could be a credential,
        // short of the caller's own configured location, and short of what one
        // event may carry. What Drive actually answered is the whole point of
        // recording a refusal: paraphrasing it into a category of ours is
        // exactly what loses the evidence.
        let body = redact::body_without(&bytes, private);

        let envelope: Option<ErrorEnvelope> = serde_json::from_slice(&bytes).ok();
        let reason = envelope
            .as_ref()
            .and_then(|envelope| envelope.error.errors.as_ref())
            .and_then(|errors| errors.first())
            .and_then(|first| first.reason.clone())
            .unwrap_or_default();

        // Drive writes its message for whoever asked, so it quotes the folder
        // it was pointed at as readily as the body around it does. The fallback
        // is the body, which has already been through the same rule.
        let detail = envelope
            .as_ref()
            .and_then(|envelope| envelope.error.message.as_deref())
            .map(|message| redact::text_without(message, private))
            .unwrap_or_else(|| body.clone());

        Self {
            operation,
            status,
            reason,
            detail,
            body,
            retry_after,
        }
    }

    /// What the failure of a call that addresses one object means to the port.
    pub fn into_object_error(self, name: &str) -> Error {
        self.into_error(Missing::Object(name.to_owned()))
    }

    /// What the failure means to the port.
    ///
    /// `missing` is what the call asked Drive for, and it is the caller's to
    /// say: a refusal does not know whether the request was about one object,
    /// about the folder somebody pointed this Library at, or about an endpoint
    /// that names nothing of the Library's at all. It reaches the not-found
    /// event through [`Missing::subject`], which is what keeps a chosen folder
    /// out of that event while leaving the field one word per kind.
    pub fn into_error(self, missing: Missing) -> Error {
        let Self {
            operation,
            status,
            reason,
            detail,
            body,
            retry_after,
        } = self;

        // Keeps what Drive answered, for whoever comes to read the log. Only
        // the answers that fall into a catch-all below are recorded: those are
        // the ones the port has no state for, so the code above can do nothing
        // but report them — and the next person asking "what does Drive
        // actually send when this happens?" has only this to go on. What goes
        // in is the operation coffret named and the status, reason and body
        // Drive answered with, the body having had any credential taken out of
        // it. Opacity alone is not what makes that safe — a provider may echo
        // any part of a request (spec: EL-5).
        let record = |what: &str| {
            warn!(
                operation,
                status,
                reason = %reason,
                body = %body,
                "{what}",
            );
        };

        match status {
            401 => Error::Unauthenticated { detail },
            403 if THROTTLING_REASONS.contains(&reason.as_str()) => Error::RateLimited {
                retry_after,
                detail,
            },
            // Classified rather than recorded: a limit Drive names is one this
            // build already understands, and the catch-all below stays for the
            // reasons it does not.
            403 if LIMIT_REASONS.contains(&reason.as_str()) => Error::LimitReached {
                limit: reason.clone(),
                detail,
            },
            403 => {
                record("Storage refused access");
                Error::PermissionDenied { detail }
            }
            404 => {
                // Not a fault, and not an error-level event: a fresh Library, an
                // interrupted rotation, and an ordinary probe all look like
                // this, and none of them is anything a person has to act on.
                debug!(
                    operation,
                    object = missing.subject(),
                    "Storage holds no such object"
                );
                Error::NotFound { missing }
            }
            416 => Error::Unsupported { detail },
            429 => Error::RateLimited {
                retry_after,
                detail,
            },
            500..=599 => Error::ServiceUnavailable { status, detail },
            // A create with no race to lose, finding the name taken. It is the
            // refusal its status says it is, and it is also a contradiction:
            // either something coffret did not put there is in Storage, or a
            // minted identifier was spent twice.
            _ if status == CONFLICT || reason == DUPLICATE_REASON => {
                record("a create that could not have lost a race found the name taken");
                Error::Rejected { status, detail }
            }
            _ => {
                record("Storage rejected the request");
                Error::Rejected { status, detail }
            }
        }
    }

    /// What the failure of a conditional create means to the port.
    ///
    /// A create that names an already-used identifier is a lost commit race,
    /// not a fault: the caller refreshes the control head and retries rather
    /// than concluding it cannot write. Drive reports it as a conflict, or as a
    /// 4xx whose reason is `duplicate`. Nothing is recorded for it — losing a
    /// race is the commit protocol working.
    pub fn into_conditional_create_error(self, name: &str) -> Error {
        if self.status == CONFLICT || self.reason == DUPLICATE_REASON {
            return Error::AlreadyExists {
                object: name.to_owned(),
            };
        }
        self.into_object_error(name)
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
        let error = FailedResponse::read(response, "put", &PrivateValues::none())
            .await
            .into_object_error("head-1.cfrt");

        assert!(matches!(error, Error::RateLimited { .. }));
        assert!(error.is_retryable());
    }

    #[tokio::test]
    async fn a_genuine_refusal_stays_a_refusal() {
        let response = refusal(403, &envelope("insufficientFilePermissions", "No access."));
        let error = FailedResponse::read(response, "put", &PrivateValues::none())
            .await
            .into_object_error("head-1.cfrt");

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
        let error = FailedResponse::read(response, "put", &PrivateValues::none())
            .await
            .into_object_error("head-1.cfrt");

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
        let error = FailedResponse::read(response, "put_if_absent", &PrivateValues::none())
            .await
            .into_conditional_create_error("head-1.cfrt");

        match &error {
            Error::AlreadyExists { object } => assert_eq!(object, "head-1.cfrt"),
            other => panic!("expected a lost conditional create, got {other:?}"),
        }
    }

    /// A body that would go on for as long as anyone read it.
    ///
    /// What a refusal of a `get` could carry if the answer were believed: that
    /// call's successful answers are Storage Objects, so nothing about its size
    /// is small by nature, and a refusal is read before anything has been
    /// authenticated.
    struct Endless;

    impl tokio::io::AsyncRead for Endless {
        fn poll_read(
            self: std::pin::Pin<&mut Self>,
            _context: &mut std::task::Context<'_>,
            buf: &mut tokio::io::ReadBuf<'_>,
        ) -> std::task::Poll<std::io::Result<()>> {
            let take = buf.remaining();
            buf.initialize_unfilled_to(take);
            buf.advance(take);
            std::task::Poll::Ready(Ok(()))
        }
    }

    // A refusal is read for what it explains, and a document's worth of it is
    // all that can explain anything. An answer that keeps going is stopped
    // there rather than followed — a reader that believed the declaration would
    // still be reading.
    #[tokio::test]
    async fn a_refusal_that_never_ends_is_read_only_as_far_as_a_document() {
        let response = HttpResponse::new(
            500,
            Vec::new(),
            ByteStream::new(u64::from(u32::MAX), Endless),
        );
        let failure = FailedResponse::read(response, "get", &PrivateValues::none()).await;

        // Not Drive's envelope, so it classifies by status alone — and it got
        // there at all, which is the point.
        let error = failure.into_object_error("head-1.cfrt");
        assert!(matches!(
            error,
            Error::ServiceUnavailable { status: 500, .. }
        ));
    }

    // Drive quotes what it was asked for, and what a pre-store call asks for is
    // a folder somebody picked out of their own Drive. It is taken out of the
    // body and out of the message the envelope carries, while everything else
    // Drive said about the refusal is left where it is (spec: EL-5).
    #[tokio::test]
    async fn a_body_that_echoes_a_private_location_is_read_without_it() {
        let chosen = "Family Archive 2019";
        let response = refusal(
            403,
            &envelope(
                "insufficientFilePermissions",
                &format!("No write access to {chosen} for this app."),
            ),
        );

        let failure = FailedResponse::read(
            response,
            "create_app_folder",
            &PrivateValues::none().with(chosen),
        )
        .await;

        assert!(!failure.body.contains(chosen), "{}", failure.body);
        assert!(!failure.detail.contains(chosen), "{}", failure.detail);
        // The refusal is still a refusal, and still says what kind.
        assert_eq!(failure.reason, "insufficientFilePermissions");
        assert!(
            failure.body.contains("No write access to"),
            "{}",
            failure.body
        );
        assert!(
            failure.detail.contains("for this app."),
            "{}",
            failure.detail
        );
    }

    #[tokio::test]
    async fn a_body_that_is_not_drives_envelope_still_classifies_by_status() {
        let response = refusal(503, "<html>backend error</html>");
        let error = FailedResponse::read(response, "get", &PrivateValues::none())
            .await
            .into_object_error("head-1.cfrt");

        assert!(matches!(
            error,
            Error::ServiceUnavailable { status: 503, .. }
        ));
        assert!(error.is_retryable());
    }
}
