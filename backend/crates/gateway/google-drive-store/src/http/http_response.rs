use coffret_usecase::ByteStream;

/// What Drive answered.
///
/// The body stays a stream, because a `get` of a Container is answered here and
/// collecting it would put the object in memory just to hand it back out again.
pub struct HttpResponse {
    status: u16,
    headers: Vec<(String, String)>,
    body: ByteStream,
}

impl HttpResponse {
    /// Takes a status, headers, and a body.
    pub fn new(status: u16, headers: Vec<(String, String)>, body: ByteStream) -> Self {
        Self {
            status,
            headers,
            body,
        }
    }

    /// The status code.
    pub fn status(&self) -> u16 {
        self.status
    }

    /// Whether the status is a 2xx.
    pub fn is_success(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// The value of a header, matched without regard to case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// The body, to read or to hand on.
    pub fn into_body(self) -> ByteStream {
        self.body
    }
}

#[cfg(test)]
impl HttpResponse {
    /// A response carrying a JSON document, for a stub transport to answer with.
    pub fn json(status: u16, body: &str) -> Self {
        Self::new(
            status,
            vec![("content-type".to_owned(), "application/json".to_owned())],
            ByteStream::from(body.as_bytes()),
        )
    }
}
