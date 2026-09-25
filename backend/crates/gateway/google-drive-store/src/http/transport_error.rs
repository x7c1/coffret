use std::error;
use std::fmt;
use std::sync::Arc;

use coffret_usecase::GatewayFailure;

/// What the transport reported, as the value it reported it in.
///
/// A client library's error for a call that went wrong, or the operating
/// system's for a body that stopped arriving — whichever layer saw it. Kept as
/// the value rather than as its message, because the message is the least of
/// what it knows: reqwest's error answers whether it was a timeout, a connect
/// or a body failure, and has links of its own underneath that say which host
/// refused and why. Shared behind an [`Arc`] because [`TransportError`] is
/// [`Clone`] and neither of those errors is.
pub type TransportCause = Arc<dyn error::Error + Send + Sync + 'static>;

/// A failure that happened instead of an answer.
///
/// Anything Drive said, however unwelcome, is an
/// [`HttpResponse`](crate::http::HttpResponse); this is only for calls that
/// never became one. The distinction is what keeps "Drive refused" and "the
/// network refused" from being told apart by reading a message.
///
/// Three of the variants are about the call and two about the answer that came
/// over it. They are kept apart because nothing a person does about one bears
/// on the other, and a difference that matters that much is not left to a
/// message.
#[derive(Debug, Clone)]
pub enum TransportError {
    /// The call ran out of time.
    Timeout {
        /// What the transport reported.
        cause: TransportCause,
    },
    /// The call never reached Drive: DNS, TLS, or the connection itself.
    Connect {
        /// What the transport reported.
        cause: TransportCause,
    },
    /// The connection broke while the body was moving.
    ///
    /// Only that. An answer this gateway refused for what it was is one of the
    /// two below, so a caller reading this one is being told about the network
    /// and nothing else.
    Body {
        /// What the transport reported.
        cause: TransportCause,
    },
    /// The answer declared no length and ran past the most of it this call said
    /// it would take in.
    ///
    /// Nothing went wrong in transit: the bytes were arriving, and there were
    /// more of them than any answer to this call could be. The ceiling travels
    /// in the refusal because it is the caller's own number — what the document
    /// it asked for can come to — and somebody deciding what to do next cannot
    /// act on a limit they are not told.
    AnswerTooLong {
        /// The most of the answer this call would take in.
        ceiling: u64,
    },
    /// An answer carrying a Storage Object's bytes arrived without a length.
    ///
    /// Drive declares one on every such answer, and this is what happens where
    /// something did not — a proxy on the path, a provider having a bad day, or
    /// something standing in for Drive entirely, none of which is inside the
    /// trust boundary. Collecting the answer to find out how long it is would
    /// mean holding a whole Container in memory, which is the one thing the
    /// port's streams exist to avoid; measuring it against a ceiling meant for
    /// JSON would refuse almost every real object. So it is refused for what it
    /// actually is.
    UndeclaredObjectLength,
}

impl fmt::Display for TransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Which of the three it was, and not what the transport said: that
            // is the value `source` hands on, and a caller walking the chain
            // prints it under this line rather than twice over.
            Self::Timeout { .. } => f.write_str("the call timed out"),
            Self::Connect { .. } => f.write_str("the call could not be made"),
            Self::Body { .. } => f.write_str("the body could not be transferred"),
            Self::AnswerTooLong { ceiling } => write!(
                f,
                "an answer carrying no length ran past the {ceiling} bytes this call takes in"
            ),
            Self::UndeclaredObjectLength => write!(
                f,
                "an answer carrying a Storage Object's bytes declared no length, \
                 and no ceiling of this gateway's is one to measure an object against"
            ),
        }
    }
}

impl error::Error for TransportError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Timeout { cause } | Self::Connect { cause } | Self::Body { cause } => {
                Some(cause.as_ref())
            }
            // Nothing a Rust error reported: both are this gateway's own
            // verdict on an answer that did arrive.
            Self::AnswerTooLong { .. } | Self::UndeclaredObjectLength => None,
        }
    }
}

impl From<TransportError> for coffret_usecase::Error {
    fn from(error: TransportError) -> Self {
        // The value crosses as the port's `source`, and what the transport
        // said is the link under it, so `detail` is this error's own line and
        // no more: rendering the transport's message into it as well would
        // spell that message twice in any chain printed from the port.
        let detail = error.to_string();
        // Matched in place, so that the arms bind nothing and the error is
        // still whole to hand over.
        match error {
            TransportError::Timeout { .. } => Self::Timeout {
                detail,
                source: Some(GatewayFailure::new(error)),
            },
            // A call that never landed and one that broke halfway are both worth
            // making again: neither says anything about the state of Storage.
            TransportError::Connect { .. } | TransportError::Body { .. } => Self::Transport {
                detail,
                source: Some(GatewayFailure::new(error)),
            },
            // Neither of these is a transfer that went wrong: the bytes were
            // arriving, and what arrived is not an answer to this call. "An
            // answer this build cannot read" is the only word the port has for
            // that — calling either one a transport failure would say the call
            // never reached Storage, which it did — and it is a word a caller
            // reports rather than loops on. For an answer past a ceiling that
            // follows, because it is past the same ceiling on the next call.
            // For one carrying an object and no length it is a judgement: a
            // length that is missing is far more often a path that always drops
            // it than a moment that passes, and it is the judgement the S3
            // gateway already makes of the same answer.
            TransportError::AnswerTooLong { .. } | TransportError::UndeclaredObjectLength => {
                Self::MalformedResponse {
                    detail,
                    source: Some(GatewayFailure::new(error)),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io;

    use super::*;

    // What the split is for, read from the outside: the two refusals about the
    // answer are the ones that changed hands, an over-long body having crossed
    // as a broken connection before and so been asked for again. Neither is
    // worth asking again for, each for the reason on its arm of the conversion
    // above — and the retry loop reads that off the variant alone, never off
    // the message.
    #[test]
    fn an_answer_this_call_cannot_read_is_not_worth_asking_for_again() {
        for refusal in [
            TransportError::AnswerTooLong { ceiling: 1024 },
            TransportError::UndeclaredObjectLength,
        ] {
            let crossed = coffret_usecase::Error::from(refusal.clone());
            assert!(
                matches!(crossed, coffret_usecase::Error::MalformedResponse { .. }),
                "no later attempt makes {refusal} readable: {crossed:?}",
            );
            assert!(!crossed.is_retryable(), "{crossed}");
        }
    }

    // And the other half of the split is unchanged: a call that never landed or
    // broke on the way says nothing about the state of Storage, so a later one
    // may still be answered.
    #[test]
    fn a_call_that_failed_on_the_way_is_still_worth_another_attempt() {
        for failure in [
            TransportError::Timeout {
                cause: Arc::new(io::Error::new(io::ErrorKind::TimedOut, "no answer in 60s")),
            },
            TransportError::Connect {
                cause: Arc::new(io::Error::from(io::ErrorKind::ConnectionRefused)),
            },
            TransportError::Body {
                cause: Arc::new(io::Error::from(io::ErrorKind::ConnectionReset)),
            },
        ] {
            let crossed = coffret_usecase::Error::from(failure);
            assert!(crossed.is_retryable(), "{crossed}");
        }
    }
}
