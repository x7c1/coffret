use std::fmt;
use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::{Error, Result};

/// The bytes of one object, travelling in either direction, with the length
/// known up front.
///
/// Storage Objects are as large as the files they carry, so the port moves them
/// as streams rather than buffers: nothing here ever holds a whole Container in
/// memory. The length rides along because every provider wants it before the
/// first byte — S3 signs a `Content-Length`, Drive opens a resumable session
/// with one — and because it is what lets a short transfer be caught as
/// [`Error::LengthMismatch`] instead of silently truncating an object.
pub struct ByteStream {
    len: u64,
    reader: Pin<Box<dyn AsyncRead + Send + 'static>>,
}

impl ByteStream {
    /// Takes a reader that will yield exactly `len` bytes.
    pub fn new(len: u64, reader: impl AsyncRead + Send + 'static) -> Self {
        Self {
            len,
            reader: Box::pin(reader),
        }
    }

    /// How many bytes the stream carries.
    pub fn len(&self) -> u64 {
        self.len
    }

    /// Whether the stream carries no bytes at all.
    ///
    /// A zero-length object is a legitimate object, not a missing one.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Unwraps the reader, for a gateway handing the bytes to its SDK.
    pub fn into_reader(self) -> Pin<Box<dyn AsyncRead + Send + 'static>> {
        self.reader
    }

    /// Drains the stream into memory, checking it was as long as it claimed.
    ///
    /// Only for objects a caller knows are small — control objects, test
    /// fixtures. Container content goes through [`ByteStream::into_reader`].
    pub async fn into_bytes(self) -> Result<Vec<u8>> {
        let expected = self.len;
        let mut bytes = Vec::with_capacity(usize::try_from(expected).unwrap_or(0));
        let mut reader = self.reader;
        reader.read_to_end(&mut bytes).await?;

        let actual = bytes.len() as u64;
        if actual == expected {
            Ok(bytes)
        } else {
            Err(Error::LengthMismatch { expected, actual })
        }
    }
}

impl From<Vec<u8>> for ByteStream {
    fn from(bytes: Vec<u8>) -> Self {
        Self::new(bytes.len() as u64, std::io::Cursor::new(bytes))
    }
}

impl From<&[u8]> for ByteStream {
    fn from(bytes: &[u8]) -> Self {
        Self::from(bytes.to_vec())
    }
}

impl fmt::Debug for ByteStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The reader is a trait object with nothing to show, and draining it to
        // print it would consume the very bytes the caller is about to send.
        f.debug_struct("ByteStream")
            .field("len", &self.len)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn round_trips_bytes_through_memory() {
        let stream = ByteStream::from(b"the object's bytes".to_vec());
        assert_eq!(stream.len(), 18);
        assert_eq!(stream.into_bytes().await.unwrap(), b"the object's bytes");
    }

    #[tokio::test]
    async fn a_zero_length_stream_is_empty_not_missing() {
        let stream = ByteStream::from(Vec::new());
        assert!(stream.is_empty());
        assert_eq!(stream.into_bytes().await.unwrap(), Vec::<u8>::new());
    }

    #[tokio::test]
    async fn a_short_reader_is_caught_rather_than_truncating() {
        let stream = ByteStream::new(64, std::io::Cursor::new(b"only ten b".to_vec()));
        assert_eq!(
            stream.into_bytes().await,
            Err(Error::LengthMismatch {
                expected: 64,
                actual: 10,
            })
        );
    }
}
