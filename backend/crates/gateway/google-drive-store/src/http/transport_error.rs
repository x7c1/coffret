use std::fmt;

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
        detail: String,
    },
    /// The call never reached Drive: DNS, TLS, or the connection itself.
    Connect {
        /// What the transport reported.
        detail: String,
    },
    /// The connection broke while the body was moving.
    ///
    /// Only that. An answer this gateway refused for what it was is one of the
    /// two below, so a caller reading this one is being told about the network
    /// and nothing else.
    Body {
        /// What the transport reported.
        detail: String,
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
            Self::Timeout { detail } => write!(f, "the call timed out: {detail}"),
            Self::Connect { detail } => write!(f, "the call could not be made: {detail}"),
            Self::Body { detail } => write!(f, "the body could not be transferred: {detail}"),
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
                Self::MalformedResponse { detail }
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
                detail: "no answer in 60s".to_owned(),
            },
            TransportError::Connect {
                detail: "connection refused".to_owned(),
            },
            TransportError::Body {
                detail: "connection reset".to_owned(),
            },
        ] {
            let crossed = coffret_usecase::Error::from(failure);
            assert!(crossed.is_retryable(), "{crossed}");
        }
    }
}
