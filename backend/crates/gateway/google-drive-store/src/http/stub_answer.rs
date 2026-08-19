use crate::http::http_response::HttpResponse;
use crate::http::transport_error::TransportError;

/// What the stub answers one call with.
pub enum StubAnswer {
    /// Drive answered this.
    Respond(HttpResponse),
    /// The call never became an answer.
    Fail(TransportError),
}

impl StubAnswer {
    /// A JSON answer with a status.
    pub fn json(status: u16, body: &str) -> Self {
        Self::Respond(HttpResponse::json(status, body))
    }

    /// A JSON answer carrying headers as well.
    pub fn json_with_headers(status: u16, headers: Vec<(String, String)>, body: &str) -> Self {
        Self::Respond(HttpResponse::new(
            status,
            headers,
            coffret_usecase::ByteStream::from(body.as_bytes()),
        ))
    }
}
