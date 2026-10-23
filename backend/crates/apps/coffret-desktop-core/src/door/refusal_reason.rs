use std::error::Error as _;
use std::io;

/// Why the door refused a request, in fixed words only (spec: EL-1, EL-3,
/// EL-4).
///
/// The reason is the fence or the limit the request met. A body that was not a
/// submission says which way it failed as well, because "not JSON" alone does
/// not tell a truncated upload from a malformed one: the category the JSON
/// reader filed it under, or `broken` for a frame the connection broke off,
/// and for a failure of I/O underneath, its kind. Never the error's own
/// rendering, or the line and column it names: those may quote the body, and
/// the body is a Passphrase.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RefusalReason {
    /// A fixed word naming the fence or the limit.
    pub(crate) word: &'static str,
    /// Which way a body failed to be read.
    failure: Option<Failure>,
    /// The kind of the I/O failure underneath, where there was one.
    pub(crate) io_kind: Option<io::ErrorKind>,
}

/// Which way a body failed to be read, kept as a tag rather than as its word so
/// that a refusal stays small enough to travel by value.
#[derive(Debug, Clone, Copy)]
enum Failure {
    Io,
    Syntax,
    Data,
    Eof,
    Broken,
}

impl RefusalReason {
    /// A body the JSON reader refused, by the category it filed it under and,
    /// for an I/O failure, that failure's kind.
    pub(crate) fn unread_json(word: &'static str, error: &serde_json::Error) -> Self {
        use serde_json::error::Category;
        let failure = match error.classify() {
            Category::Io => Failure::Io,
            Category::Syntax => Failure::Syntax,
            Category::Data => Failure::Data,
            Category::Eof => Failure::Eof,
        };
        Self {
            word,
            failure: Some(failure),
            io_kind: error.io_error_kind(),
        }
    }

    /// A body whose frame broke off on the way in.
    ///
    /// The transport's error has no kind of its own to give, so it is
    /// `broken`, with the kind of the I/O failure underneath where the chain
    /// holds one.
    pub(crate) fn broken_frame(word: &'static str, error: &axum::Error) -> Self {
        let mut io_kind = None;
        let mut next = error.source();
        while let Some(cause) = next {
            if let Some(io) = cause.downcast_ref::<io::Error>() {
                io_kind = Some(io.kind());
                break;
            }
            next = cause.source();
        }
        Self {
            word,
            failure: Some(Failure::Broken),
            io_kind,
        }
    }
}

impl RefusalReason {
    /// Which way a body failed to be read: `io`, `syntax`, `data` or `eof` as
    /// the JSON reader classifies it, or `broken` for a frame that broke.
    pub(crate) fn failure(&self) -> Option<&'static str> {
        self.failure.map(|failure| match failure {
            Failure::Io => "io",
            Failure::Syntax => "syntax",
            Failure::Data => "data",
            Failure::Eof => "eof",
            Failure::Broken => "broken",
        })
    }
}

impl From<&'static str> for RefusalReason {
    fn from(word: &'static str) -> Self {
        Self {
            word,
            failure: None,
            io_kind: None,
        }
    }
}
