use coffret_usecase::ByteStream;

use crate::http::method::Method;
use crate::http::request_body::RequestBody;

/// One call to make against Drive.
///
/// Requests are values rather than calls made through a client, which is what
/// lets a test hand the gateway a transport that inspects them and answers
/// whatever the case needs — an injected 429, a mismatched digest — without a
/// network, a mock server, or a build-time switch.
pub struct HttpRequest {
    /// The method to call with.
    pub method: Method,
    /// The full URL, query string included.
    pub url: String,
    /// The headers to send, in the order they were added.
    pub headers: Vec<(String, String)>,
    /// The body to send.
    pub body: RequestBody,
}

impl HttpRequest {
    /// A request with no headers and no body.
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: Vec::new(),
            body: RequestBody::Empty,
        }
    }

    /// Adds a header.
    pub fn with_header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Sends a JSON document as the body.
    pub fn with_json(mut self, json: &serde_json::Value) -> Self {
        self.body = RequestBody::Bytes(json.to_string().into_bytes());
        self.with_header("content-type", "application/json; charset=UTF-8")
    }

    /// Sends a Storage Object's bytes as the body.
    pub fn with_stream(mut self, body: ByteStream) -> Self {
        self.body = RequestBody::Stream(body);
        self
    }

    /// The value of a header, matched without regard to case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}
