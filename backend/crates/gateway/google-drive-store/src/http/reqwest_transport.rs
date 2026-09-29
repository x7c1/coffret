use std::io::Cursor;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use coffret_usecase::ByteStream;
use futures_util::TryStreamExt;
use tokio::io::AsyncReadExt;
use tokio_util::io::{ReaderStream, StreamReader};

use crate::error::{Error, Result};
use crate::http::answer_body::answer_body;
use crate::http::expected_answer::ExpectedAnswer;
use crate::http::http_request::HttpRequest;
use crate::http::http_response::HttpResponse;
use crate::http::http_transport::HttpTransport;
use crate::http::method::Method;
use crate::http::request_body::RequestBody;
use crate::http::transport_error::TransportError;

/// How long to wait for a connection to be established.
///
/// Every call pays it before its first byte, so it is inside
/// [`WHOLE_CALL_DEADLINE`] with room to spare: a connection that takes the
/// whole of it still leaves a small call time to be answered.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// How long to wait between bytes once a transfer is running.
///
/// This is the only bound on a call that streams a Storage Object's bytes — the
/// upload of one, or the answer carrying one. There is deliberately no deadline
/// on the whole of such a call: an upload of a large Container is legitimately
/// slow, and only a stalled one is a failure. It is also the wait for an answer
/// to begin, since nothing arrives before the first byte of one either.
pub const READ_TIMEOUT: Duration = Duration::from_secs(60);

/// How long a call whose body is small may take altogether, from opening the
/// connection to the last byte of its answer.
///
/// Every call that does not stream a Storage Object's bytes is one of these: a
/// listing, a file resource, the minting of an identifier, the opening of an
/// upload session, a trash or delete, the probes a join makes, a token refresh.
/// Their answers are a few kilobytes at most (see `answer_ceiling`), so the
/// between-bytes [`READ_TIMEOUT`] alone would let one run forever — an answer
/// that trickles a byte a minute never trips it — and a call that never ends is
/// a failure the retry policy cannot see, because it only acts on failures that
/// arrive. With this, a stalled small call arrives as
/// [`TransportError::Timeout`], which the policy already retries.
///
/// Thirty seconds is past the [`CONNECT_TIMEOUT`] with ten to spare for the
/// answer, which is orders of magnitude more than a document of this size needs
/// on any link that works at all. It is half the server's minute-long start-up
/// catch-up wait (`catch_up_at_startup::DEADLINE` in `coffret-server`, which the
/// e2e suite's start-up wait is measured against): a call that stalls while the
/// explorer is catching up is given up on, and made again, inside that minute
/// rather than outliving it. The S3 gateway uses the same figure
/// (`s3_store::SMALL_CALL_DEADLINE`), so a slow Storage is judged alike whichever
/// provider it is.
pub const WHOLE_CALL_DEADLINE: Duration = Duration::from_secs(30);

/// The transport that actually talks to Google.
///
/// Everything provider-specific about it is here rather than in the gateway:
/// the gateway builds requests and reads answers, and this turns them into
/// sockets.
///
/// Which bound a call gets is read off the request's shape, because the shape
/// is what says whether the call can legitimately be slow: one that streams a
/// Storage Object's bytes out ([`RequestBody::Stream`]) or asks for them back
/// ([`ExpectedAnswer::ObjectBytes`]) is held only between bytes, and every other
/// call is held to [`WHOLE_CALL_DEADLINE`] as well.
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
    whole_call_deadline: Duration,
}

impl ReqwestTransport {
    /// Takes a client configured by the caller.
    ///
    /// Small calls are held to [`WHOLE_CALL_DEADLINE`]; the client's own
    /// timeouts are the caller's to choose.
    pub fn new(client: reqwest::Client) -> Self {
        Self {
            client,
            whole_call_deadline: WHOLE_CALL_DEADLINE,
        }
    }

    /// Builds a client with the timeouts this gateway expects.
    pub fn with_default_client() -> Result<Self> {
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
            .map_err(|cause| Error::HttpClient { cause })?;

        Ok(Self::new(client))
    }

    /// Holds small calls to another deadline than [`WHOLE_CALL_DEADLINE`].
    ///
    /// Calls that stream a Storage Object's bytes are unaffected.
    pub fn with_whole_call_deadline(mut self, deadline: Duration) -> Self {
        self.whole_call_deadline = deadline;
        self
    }

    /// Makes the call and reads the head of its answer, leaving the body to
    /// whoever drains it.
    async fn perform(
        &self,
        request: HttpRequest,
    ) -> std::result::Result<HttpResponse, TransportError> {
        // Read off the request before it is consumed by being sent.
        let expected = request.answer;
        let mut builder = self
            .client
            .request(to_reqwest_method(request.method), &request.url);

        for (name, value) in &request.headers {
            builder = builder.header(name, value);
        }

        builder =
            match request.body {
                RequestBody::Empty => builder,
                RequestBody::Bytes(bytes) => builder.body(bytes),
                RequestBody::Stream(stream) => {
                    // Drive wants the length before the first byte of a resumable
                    // upload, and the stream knows it, so say it rather than letting
                    // the body go out chunked.
                    let len = stream.len();
                    builder.header("content-length", len.to_string()).body(
                        reqwest::Body::wrap_stream(ReaderStream::new(stream.into_reader())),
                    )
                }
            };

        let response = builder.send().await.map_err(classify)?;

        let status = response.status().as_u16();
        let headers = response
            .headers()
            .iter()
            .filter_map(|(name, value)| {
                Some((name.as_str().to_owned(), value.to_str().ok()?.to_owned()))
            })
            .collect();

        // What an answer without a length costs is decided by what the call
        // asked for, and nothing here knows that — so the request said it, and
        // the decision is made where both the shape and the ceiling are.
        let declared = response.content_length();
        let bytes = response.bytes_stream().map_err(std::io::Error::other);
        let body = answer_body(expected, status, declared, StreamReader::new(bytes)).await?;

        Ok(HttpResponse::new(status, headers, body))
    }

    /// Makes a small call, answer and all, inside the whole-call deadline.
    ///
    /// The answer is read here rather than by whoever asked for it, because a
    /// deadline that ended with the head of the answer would leave the body —
    /// the part a trickling answer is slow in — outside it.
    async fn perform_within_deadline(
        &self,
        request: HttpRequest,
    ) -> std::result::Result<HttpResponse, TransportError> {
        let ceiling = match request.answer {
            ExpectedAnswer::Document { within } => within,
            // Never asked of a small call: `streams` sends it the other way.
            ExpectedAnswer::ObjectBytes => u64::MAX,
        };
        let call = async {
            let response = self.perform(request).await?;
            buffer(response, ceiling).await
        };
        match tokio::time::timeout(self.whole_call_deadline, call).await {
            Ok(answered) => answered,
            Err(elapsed) => Err(TransportError::Timeout {
                cause: Arc::new(elapsed),
            }),
        }
    }
}

/// Whether a call moves a Storage Object's bytes, and so may take as long as
/// they keep arriving.
fn streams(request: &HttpRequest) -> bool {
    matches!(request.body, RequestBody::Stream(_))
        || matches!(request.answer, ExpectedAnswer::ObjectBytes)
}

/// Drains a small call's answer into memory, keeping what it declared.
///
/// Held to what it declared plus one byte, or to the call's ceiling plus one
/// where that is less, so a body that never ends costs a bounded amount of
/// memory as well as of time. What arrived is handed on under the length the
/// answer declared, not the length that arrived: judging the two against each
/// other stays with whoever reads the answer, exactly as when the body was a
/// live stream.
async fn buffer(
    response: HttpResponse,
    ceiling: u64,
) -> std::result::Result<HttpResponse, TransportError> {
    let (status, headers, body) = response.into_parts();
    let declared = body.len();
    let mut collected = Vec::new();
    body.into_reader()
        .take(declared.min(ceiling).saturating_add(1))
        .read_to_end(&mut collected)
        .await
        .map_err(classify_read)?;
    Ok(HttpResponse::new(
        status,
        headers,
        ByteStream::new(declared, Cursor::new(collected)),
    ))
}

/// Which kind of failure a client error was.
///
/// The client's error is asked which kind it was and then kept as it is: what
/// it says, and the links it keeps under that, are the transport's own account
/// of the failure, and rendering it here would leave only the first of them.
fn classify(error: reqwest::Error) -> TransportError {
    if error.is_timeout() {
        TransportError::Timeout {
            cause: Arc::new(error),
        }
    } else if error.is_body() || error.is_decode() {
        TransportError::Body {
            cause: Arc::new(error),
        }
    } else {
        TransportError::Connect {
            cause: Arc::new(error),
        }
    }
}

/// Which kind of failure a read of an answer's body was.
///
/// The body arrives as reqwest's stream behind an [`std::io::Error`], so the
/// client's error is looked for inside it: the between-bytes timeout firing
/// while an answer is drained is a [`TransportError::Timeout`] like any other,
/// not a broken connection. Either way the error is kept as it arrived.
fn classify_read(cause: std::io::Error) -> TransportError {
    let timed_out = cause
        .get_ref()
        .and_then(|inner| inner.downcast_ref::<reqwest::Error>())
        .is_some_and(reqwest::Error::is_timeout);
    if timed_out {
        TransportError::Timeout {
            cause: Arc::new(cause),
        }
    } else {
        TransportError::Body {
            cause: Arc::new(cause),
        }
    }
}

/// The reqwest method for one of ours.
fn to_reqwest_method(method: Method) -> reqwest::Method {
    match method {
        Method::Get => reqwest::Method::GET,
        Method::Post => reqwest::Method::POST,
        Method::Put => reqwest::Method::PUT,
        Method::Patch => reqwest::Method::PATCH,
        Method::Delete => reqwest::Method::DELETE,
    }
}

#[async_trait]
impl HttpTransport for ReqwestTransport {
    async fn execute(
        &self,
        request: HttpRequest,
    ) -> std::result::Result<HttpResponse, TransportError> {
        if streams(&request) {
            self.perform(request).await
        } else {
            self.perform_within_deadline(request).await
        }
    }
}
