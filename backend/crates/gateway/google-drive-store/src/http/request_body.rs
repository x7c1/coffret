use coffret_usecase::ByteStream;

/// What a request carries, if anything.
///
/// Uploads keep the port's stream rather than a buffer, so a Container is never
/// held in memory on its way to Drive; everything else is a short JSON document
/// or nothing at all.
pub enum RequestBody {
    /// No body — a read, a delete, an upload session that is only being opened.
    Empty,
    /// A body short enough to hold, which in practice means JSON.
    Bytes(Vec<u8>),
    /// A Storage Object's bytes, streamed as they are sent.
    Stream(ByteStream),
}
