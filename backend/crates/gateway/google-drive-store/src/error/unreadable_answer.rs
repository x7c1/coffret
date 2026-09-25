use std::error;
use std::fmt;

use coffret_usecase::GatewayFailure;

/// A document Drive answered with that is not one this build can read.
///
/// The value a parse that failed crosses the port as. What was being read is
/// this gateway's own line, and the parser's refusal is the link under it, so
/// a chain walked from the port reads both once each. Handing over the
/// parser's error alone would drop what was being read from the chain — the
/// port prints no `detail` where a value stands behind it — and keeping that
/// in a `detail` rendered with the parser's message beside it would say
/// again, in a field, what the next link already says.
#[derive(Debug)]
pub(crate) struct UnreadableAnswer {
    /// What was being read, as the words after "unreadable".
    about: String,
    /// What the parser refused.
    cause: serde_json::Error,
}

impl UnreadableAnswer {
    pub(crate) fn new(about: impl Into<String>, cause: serde_json::Error) -> Self {
        Self {
            about: about.into(),
            cause,
        }
    }

    /// The port's word for an answer this build cannot read, with this value
    /// behind it and its own top line as the `detail`.
    pub(crate) fn into_port(self) -> coffret_usecase::Error {
        coffret_usecase::Error::MalformedResponse {
            detail: self.to_string(),
            source: Some(GatewayFailure::new(self)),
        }
    }
}

impl fmt::Display for UnreadableAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unreadable {}", self.about)
    }
}

impl error::Error for UnreadableAnswer {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        Some(&self.cause)
    }
}
