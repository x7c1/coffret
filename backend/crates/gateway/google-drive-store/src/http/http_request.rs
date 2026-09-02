use coffret_usecase::ByteStream;

use crate::answer_ceiling::MAX_DOCUMENT_LEN;
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
    /// The most of an answer this call is willing to take into memory when the
    /// answer arrives without a length of its own.
    ///
    /// An answer that declares its length is handed back as a stream and held
    /// against that length by whoever drains it, so this does not bind it. An
    /// answer that declares none has to be collected before it can become one,
    /// and collecting it is spending memory on a length nobody stated — so the
    /// caller states one here instead, from what the document it asked for can
    /// be (the ceilings themselves are the gateway's own, in `answer_ceiling`).
    pub answer_within: u64,
}

impl HttpRequest {
    /// A request with no headers and no body.
    ///
    /// The answer is bounded at one JSON document's worth, which is what all but
    /// one of this gateway's calls ask for; the listing raises it with
    /// [`within`](Self::within).
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: Vec::new(),
            body: RequestBody::Empty,
            answer_within: MAX_DOCUMENT_LEN,
        }
    }

    /// Says how much of a length-less answer this call will take in.
    pub fn within(mut self, ceiling: u64) -> Self {
        self.answer_within = ceiling;
        self
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
