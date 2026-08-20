use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll};

use aws_smithy_types::body::SdkBody;
use aws_smithy_types::byte_stream::ByteStream as SdkByteStream;
use bytes::Bytes;
use http_body::{Body, Frame, SizeHint};
use tokio::io::{AsyncRead, ReadBuf};

use coffret_usecase::ByteStream;

/// How much is read from the stream per frame handed to the SDK.
///
/// Large enough that a multi-megabyte Container is not chopped into thousands
/// of frames, small enough that an upload in flight holds one buffer rather
/// than the object.
const FRAME_LEN: usize = 64 * 1024;

/// The port's stream, presented to the SDK as an HTTP body.
///
/// The bytes are pulled frame by frame as the SDK writes them to the wire, so
/// uploading a Container never holds more than one frame of user data in
/// memory.
struct ReaderBody {
    // The SDK requires a body that is `Sync`, and an `AsyncRead` is only
    // `Send`. The mutex is what bridges the two, and it is never contended:
    // the body owns its reader outright, and every poll already holds
    // `&mut self`, so it is taken by `get_mut` and never locked.
    reader: Mutex<Pin<Box<dyn AsyncRead + Send + 'static>>>,
    remaining: u64,
    finished: bool,
}

impl Body for ReaderBody {
    type Data = Bytes;
    type Error = std::io::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Bytes>, Self::Error>>> {
        let this = self.get_mut();
        if this.finished {
            return Poll::Ready(None);
        }

        let mut frame = vec![0u8; FRAME_LEN];
        let mut buffer = ReadBuf::new(&mut frame);
        let reader = this
            .reader
            .get_mut()
            .expect("a body owns its reader and never locks it");

        match reader.as_mut().poll_read(context, &mut buffer) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(error)) => Poll::Ready(Some(Err(error))),
            Poll::Ready(Ok(())) => {
                let filled = buffer.filled().len();
                if filled == 0 {
                    this.finished = true;
                    return Poll::Ready(None);
                }
                this.remaining = this.remaining.saturating_sub(filled as u64);
                frame.truncate(filled);
                Poll::Ready(Some(Ok(Frame::data(Bytes::from(frame)))))
            }
        }
    }

    fn is_end_stream(&self) -> bool {
        self.finished
    }

    fn size_hint(&self) -> SizeHint {
        // Exact, because the port carries the length with the stream and S3
        // wants a `Content-Length` before the first byte.
        SizeHint::with_exact(self.remaining)
    }
}

/// Hands the port's stream to the SDK without collecting it first.
pub fn to_sdk_stream(body: ByteStream) -> SdkByteStream {
    let remaining = body.len();
    let body = ReaderBody {
        reader: Mutex::new(body.into_reader()),
        remaining,
        finished: false,
    };
    SdkByteStream::new(SdkBody::from_body_1_x(body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn streams_every_byte_through_however_many_frames_it_takes() {
        let content: Vec<u8> = (0..FRAME_LEN * 2 + 7).map(|index| index as u8).collect();
        let stream = to_sdk_stream(ByteStream::from(content.clone()));

        let collected = stream.collect().await.expect("the body must read through");
        assert_eq!(collected.to_vec(), content);
    }

    #[tokio::test]
    async fn a_zero_length_stream_becomes_an_empty_body() {
        let stream = to_sdk_stream(ByteStream::from(Vec::new()));
        assert_eq!(stream.size_hint(), (0, Some(0)));

        let collected = stream.collect().await.expect("the body must read through");
        assert!(collected.to_vec().is_empty());
    }
}
