use crate::http::method::Method;

/// One call the gateway made, kept for a test to look at.
#[derive(Clone)]
pub struct RecordedRequest {
    /// The method it was made with.
    pub method: Method,
    /// The URL it went to.
    pub url: String,
    /// The headers it carried.
    pub headers: Vec<(String, String)>,
    /// The body it carried, drained.
    pub body: Vec<u8>,
}

impl RecordedRequest {
    /// The value of a header, matched without regard to case.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(header, _)| header.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}
