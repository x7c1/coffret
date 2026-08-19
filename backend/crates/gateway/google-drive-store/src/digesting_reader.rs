use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::{AsyncRead, ReadBuf};

use crate::upload_digest::UploadDigest;

/// A reader that hashes what it hands on.
pub struct DigestingReader<R> {
    reader: R,
    digest: UploadDigest,
}

impl<R> DigestingReader<R> {
    /// Reads from `reader`, folding everything read into `digest`.
    ///
    /// Made through [`UploadDigest::wrap`], which is where the digest that
    /// outlives the reader comes from.
    pub(crate) fn new(reader: R, digest: UploadDigest) -> Self {
        Self { reader, digest }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for DigestingReader<R> {
    fn poll_read(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let this = self.get_mut();
        let before = buffer.filled().len();
        ready!(Pin::new(&mut this.reader).poll_read(context, buffer))?;

        this.digest.update(&buffer.filled()[before..]);
        Poll::Ready(Ok(()))
    }
}
