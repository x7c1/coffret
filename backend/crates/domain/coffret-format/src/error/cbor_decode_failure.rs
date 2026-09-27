//! The CBOR decoder's refusal, carried as the value it reported.

use std::error;
use std::fmt;
use std::io;
use std::sync::Arc;

/// The CBOR decoder's refusal of bytes an object's rule spells, as the value
/// it reported.
///
/// Shared rather than copied, because [`Error`](super::Error) is `Clone` and
/// the decoder's errors are not.
#[derive(Debug, Clone)]
pub struct CborDecodeFailure(Arc<Decoder>);

/// The two points at which ciborium refuses: reading bytes into a CBOR item,
/// and turning an item into the struct a schema spells.
#[derive(Debug)]
enum Decoder {
    Reading(ciborium::de::Error<io::Error>),
    Converting(ciborium::value::Error),
}

impl CborDecodeFailure {
    pub(crate) fn reading(error: ciborium::de::Error<io::Error>) -> Self {
        Self(Arc::new(Decoder::Reading(error)))
    }

    pub(crate) fn converting(error: ciborium::value::Error) -> Self {
        Self(Arc::new(Decoder::Converting(error)))
    }
}

/// The message ciborium carries, rather than its `Debug` spelling.
///
/// ciborium displays its own errors in `Debug`, which would reach a caller as
/// `Semantic(None, "…")` or `Custom("…")` with the message quoted inside it. A
/// semantic error is the one that says something a caller can act on — which
/// field a deserializer refused, and what it was expecting there — so its
/// message is taken on its own. The remaining reading errors are ciborium's
/// own syntax, recursion and I/O errors, which carry no inner message to
/// prefer.
impl fmt::Display for CborDecodeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.as_ref() {
            Decoder::Reading(ciborium::de::Error::Semantic(_, message))
            | Decoder::Converting(ciborium::value::Error::Custom(message)) => f.write_str(message),
            Decoder::Reading(other) => other.fmt(f),
        }
    }
}

impl error::Error for CborDecodeFailure {}
