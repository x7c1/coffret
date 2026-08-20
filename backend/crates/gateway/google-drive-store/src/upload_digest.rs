use std::sync::{Arc, Mutex};

use md5::{Digest, Md5};

use crate::digesting_reader::DigestingReader;

/// The MD5 of everything that passes through a stream.
///
/// Drive reports an MD5 of what it stored, and the only way that number proves
/// anything is if the same bytes were hashed on the way out. Hashing here — as
/// they are sent, rather than by reading the object twice — is what makes the
/// comparison free and what keeps an upload from having to hold the object in
/// memory to check it.
#[derive(Debug, Clone, Default)]
pub struct UploadDigest {
    state: Arc<Mutex<Md5>>,
}

impl UploadDigest {
    /// Starts a digest over a stream not yet read.
    pub fn new() -> Self {
        Self::default()
    }

    /// Wraps a reader so that everything read from it is hashed.
    pub fn wrap<R>(&self, reader: R) -> DigestingReader<R> {
        DigestingReader::new(reader, self.clone())
    }

    /// The digest of everything read so far, as lowercase hex.
    ///
    /// Called once the stream has been read to its end, which is the point at
    /// which it is the digest of the whole object.
    pub fn to_hex(&self) -> String {
        let digest = self
            .state
            .lock()
            .expect("a digest is never held across a panic")
            .clone()
            .finalize();

        digest.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// Folds in bytes on their way past.
    pub(crate) fn update(&self, bytes: &[u8]) {
        self.state
            .lock()
            .expect("a digest is never held across a panic")
            .update(bytes);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;

    #[tokio::test]
    async fn what_is_read_is_what_is_hashed() {
        let digest = UploadDigest::new();
        let mut reader = digest.wrap(std::io::Cursor::new(b"a".to_vec()));

        let mut read = Vec::new();
        reader
            .read_to_end(&mut read)
            .await
            .expect("reading must succeed");

        assert_eq!(read, b"a");
        // The MD5 of "a", as every reference table gives it.
        assert_eq!(digest.to_hex(), "0cc175b9c0f1b6a831c399e269772661");
    }

    #[tokio::test]
    async fn an_empty_stream_hashes_to_the_empty_digest() {
        let digest = UploadDigest::new();
        let mut reader = digest.wrap(std::io::Cursor::new(Vec::new()));

        let mut read = Vec::new();
        reader
            .read_to_end(&mut read)
            .await
            .expect("reading must succeed");

        assert_eq!(digest.to_hex(), "d41d8cd98f00b204e9800998ecf8427e");
    }
}
