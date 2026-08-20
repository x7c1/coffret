use std::fmt;

/// A failure that happened instead of an answer.
///
/// Anything Drive said, however unwelcome, is an
/// [`HttpResponse`](crate::http::HttpResponse); this is only for calls that
/// never became one. The distinction is what keeps "Drive refused" and "the
/// network refused" from being told apart by reading a message.
#[derive(Debug, Clone)]
pub enum TransportError {
    /// The call ran out of time.
    Timeout {
        /// What the transport reported.
        detail: String,
    },
    /// The call never reached Drive: DNS, TLS, or the connection itself.
    Connect {
        /// What the transport reported.
        detail: String,
    },
    /// The connection broke while the body was moving.
    Body {
        /// What the transport reported.
        detail: String,
    },
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout { detail } => write!(f, "the call timed out: {detail}"),
            Self::Connect { detail } => write!(f, "the call could not be made: {detail}"),
            Self::Body { detail } => write!(f, "the body could not be transferred: {detail}"),
        }
    }
}

impl std::error::Error for TransportError {}

impl From<TransportError> for coffret_usecase::Error {
    fn from(error: TransportError) -> Self {
        let detail = error.to_string();
        match error {
            TransportError::Timeout { .. } => Self::Timeout { detail },
            // A call that never landed and one that broke halfway are both worth
            // making again: neither says anything about the state of Storage.
            TransportError::Connect { .. } | TransportError::Body { .. } => {
                Self::Transport { detail }
            }
        }
    }
}
