//! The CBOR encoder's refusal, carried as the value it reported.

use std::error;
use std::fmt;
use std::io;
use std::sync::Arc;

/// The CBOR encoder's refusal of a value this crate built, as the value it
/// reported.
///
/// Shared rather than copied, because [`Error`](super::Error) is `Clone` and
/// the encoder's errors are not.
#[derive(Debug, Clone)]
pub struct CborEncodeFailure(Arc<Encoder>);

/// The two points at which ciborium refuses on the way out: turning a struct
/// into a CBOR item, and writing an item out as bytes.
#[derive(Debug)]
enum Encoder {
    Converting(ciborium::value::Error),
    Writing(ciborium::ser::Error<io::Error>),
}

impl CborEncodeFailure {
    pub(crate) fn converting(error: ciborium::value::Error) -> Self {
        Self(Arc::new(Encoder::Converting(error)))
    }

    pub(crate) fn writing(error: ciborium::ser::Error<io::Error>) -> Self {
        Self(Arc::new(Encoder::Writing(error)))
    }
}

/// The message ciborium carries, rather than its `Debug` spelling.
///
/// ciborium displays its own errors in `Debug`, which would reach a caller as
/// `Custom("…")` or `Value("…")` with the message quoted inside it, so the
/// message is taken on its own. A failed write of the bytes is the writer's
/// own I/O error, rendered as it renders itself.
impl fmt::Display for CborEncodeFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.as_ref() {
            Encoder::Converting(ciborium::value::Error::Custom(message))
            | Encoder::Writing(ciborium::ser::Error::Value(message)) => f.write_str(message),
            Encoder::Writing(ciborium::ser::Error::Io(error)) => error.fmt(f),
        }
    }
}

impl error::Error for CborEncodeFailure {}
