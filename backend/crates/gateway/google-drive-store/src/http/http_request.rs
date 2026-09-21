use coffret_usecase::ByteStream;

use crate::http::expected_answer::ExpectedAnswer;
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
    /// What the answer to this call carries, and so what bounds it if it
    /// arrives without a length of its own.
    ///
    /// An answer that declares its length is handed back as a stream and held
    /// against that length by whoever drains it, so this does not bind it. An
    /// answer that declares none has to become one somehow, and what that costs
    /// depends on what was asked for — which is what
    /// [`ExpectedAnswer`](crate::http::ExpectedAnswer) says, and where the two
    /// cases are set out (the document ceilings themselves are the gateway's
    /// own, in `answer_ceiling`).
    pub answer: ExpectedAnswer,
}

impl HttpRequest {
    /// A request with no headers and no body.
    ///
    /// The answer is taken to be one JSON document at the ordinary ceiling,
    /// which is what all but three of this gateway's calls ask for; the two
    /// listings raise the ceiling with [`within`](Self::within), and the object
    /// fetch says what it is really asking for with
    /// [`answering_object_bytes`](Self::answering_object_bytes).
    pub fn new(method: Method, url: impl Into<String>) -> Self {
        Self {
            method,
            url: url.into(),
            headers: Vec::new(),
            body: RequestBody::Empty,
            answer: ExpectedAnswer::DOCUMENT,
        }
    }

    /// Says how much of a length-less document this call will take in.
    pub fn within(mut self, ceiling: u64) -> Self {
        self.answer = ExpectedAnswer::Document { within: ceiling };
        self
    }

    /// Says the answer is a Storage Object's bytes rather than a document.
    ///
    /// What holds those is the port's own reckoning of what the caller asked
    /// for, one layer above this gateway, so no ceiling of this one's is put on
    /// them here.
    pub fn answering_object_bytes(mut self) -> Self {
        self.answer = ExpectedAnswer::ObjectBytes;
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
