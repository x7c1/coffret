/// The HTTP methods the Drive API is reached with.
///
/// A closed set rather than a general method type: it is a short list of what
/// this gateway actually does, and it keeps a stub transport's job to matching
/// on something exhaustive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// Read a file's bytes, its metadata, or a page of a listing.
    Get,
    /// Open an upload session, mint file ids, or exchange an OAuth token.
    Post,
    /// Send the bytes of an upload session.
    Put,
    /// Change a file's metadata — trashing it, in practice.
    Patch,
    /// Delete a file for good.
    Delete,
}
