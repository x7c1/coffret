use std::time::Duration;

use async_trait::async_trait;
use coffret_usecase::ByteStream;
use futures_util::TryStreamExt;
use tokio_util::io::{ReaderStream, StreamReader};

use crate::error::{Error, Result};
use crate::http::http_request::HttpRequest;
use crate::http::http_response::HttpResponse;
use crate::http::http_transport::HttpTransport;
use crate::http::method::Method;
use crate::http::request_body::RequestBody;
use crate::http::transport_error::TransportError;

/// How long to wait for a connection to be established.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

/// How long to wait between bytes once a transfer is running.
///
/// There is deliberately no timeout on the whole call: an upload of a large
/// Container is legitimately slow, and only a stalled one is a failure.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

/// The transport that actually talks to Google.
///
/// Everything provider-specific about it is here rather than in the gateway:
/// the gateway builds requests and reads answers, and this turns them into
/// sockets.
#[derive(Debug, Clone)]
pub struct ReqwestTransport {
    client: reqwest::Client,
}

impl ReqwestTransport {
    /// Takes a client configured by the caller.
    pub fn new(client: reqwest::Client) -> Self {
        Self { client }
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
}

/// Which kind of failure a client error was.
fn classify(error: reqwest::Error) -> TransportError {
    let detail = error.to_string();
    if error.is_timeout() {
        TransportError::Timeout { detail }
    } else if error.is_body() || error.is_decode() {
        TransportError::Body { detail }
    } else {
        TransportError::Connect { detail }
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

        let body = match response.content_length() {
            Some(len) => {
                let bytes = response.bytes_stream().map_err(std::io::Error::other);
                ByteStream::new(len, StreamReader::new(bytes))
            }
            // Drive declares a length on every answer that carries a Storage
            // Object. Anything else is short enough to collect, and collecting
            // it is what lets the port keep its promise that a stream knows how
            // long it is.
            None => ByteStream::from(response.bytes().await.map_err(classify)?.to_vec()),
        };

        Ok(HttpResponse::new(status, headers, body))
    }
}
