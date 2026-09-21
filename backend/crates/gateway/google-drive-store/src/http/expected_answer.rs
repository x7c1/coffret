use crate::answer_ceiling::MAX_DOCUMENT_LEN;

/// What the answer to one call carries, and so what may bound it.
///
/// The distinction is the one `answer_ceiling`'s module doc draws, and it is
/// carried on the request because the answer cannot be asked: a body arriving
/// without a length is either a document short enough to collect or an object as
/// large as the file it holds, and nothing in the bytes says which. The caller
/// knows, because the caller chose the endpoint.
#[derive(Debug, Clone, Copy)]
pub enum ExpectedAnswer {
    /// One of the provider's own structured documents — a file resource, a page
    /// of a listing, a minted-id set, a token, an error envelope.
    ///
    /// It is small by construction, so an answer that declares no length is
    /// collected rather than refused, up to the point past which it is not the
    /// document that was asked for.
    Document {
        /// The most of it this call will take into memory when it arrives
        /// without a length of its own (see `answer_ceiling`).
        within: u64,
    },
    /// A Storage Object's bytes.
    ///
    /// As large as the file it carries, so no ceiling of this gateway's belongs
    /// on it: it is handed on as a stream, and what holds it is the port's own
    /// reckoning of what the caller asked for, one layer up. An answer of this
    /// shape that declares no length cannot become that stream and is refused,
    /// rather than collected against a number meant for JSON.
    ObjectBytes,
}

impl ExpectedAnswer {
    /// The answer every call but three asks for: the two listings want a larger
    /// ceiling, and the object fetch wants no ceiling of this gateway's at all.
    pub const DOCUMENT: Self = Self::Document {
        within: MAX_DOCUMENT_LEN,
    };
}
