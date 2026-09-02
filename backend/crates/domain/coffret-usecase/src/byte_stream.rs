use std::fmt;
use std::pin::Pin;

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::{Error, Result};

/// How much of a drain is allocated before the bytes it is for have arrived.
///
/// Reserved rather than allocated outright, so that no length commands memory
/// in advance of the bytes backing it: neither a count somebody else declared,
/// nor a ceiling this side named — a ceiling is the point past which an answer
/// is not the answer that was asked for, and never a size one is expected to
/// have. The buffer grows as the bytes actually arrive, which on an honest
/// answer is a handful of copies against a network transfer.
const RESERVE: u64 = 64 * 1024;

/// The bytes of one object, travelling in either direction, with the length
/// known up front.
///
/// Storage Objects are as large as the files they carry, so the port moves them
/// as streams rather than buffers: nothing here ever holds a whole Container in
/// memory. The length rides along because every provider wants it before the
/// first byte — S3 signs a `Content-Length`, Drive opens a resumable session
/// with one — and because it is what lets a short transfer be caught as
/// [`Error::LengthMismatch`] instead of silently truncating an object.
///
/// On the way *in*, that length is a claim and nothing more. It is Storage's
/// word about an object Storage holds, and Storage is outside the trust
/// boundary: the tag that would prove the bytes were ever this Library's is
/// inside the bytes a reader is about to hold. So the calls that drain a stream
/// into memory each say how much they are willing to spend before the first byte
/// arrives — [`into_bytes_within`](Self::into_bytes_within) against the ceiling
/// the reader brought for what it asked for, [`collect_exact`](Self::collect_exact)
/// against what a ranged read asked for, [`collect_front`](Self::collect_front)
/// against a fixed front — and an answer that ignores its own declaration is
/// stopped at the bound rather than followed. A caller that streams instead of
/// collecting spends no memory on the length, but it does spend time on it, so
/// [`into_reader`](Self::into_reader) stops at the declaration too.
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

    /// Unwraps the reader, for a caller that streams rather than collects.
    ///
    /// A gateway handing an outgoing body to its SDK, and a fetch feeding a
    /// Container past the chunk decoder onto disk: neither sizes its memory by
    /// the object, so neither needs a ceiling. What both need is an end. The
    /// reader stops one byte past the declared length, for the reason
    /// [`collect_exact`](Self::collect_exact) stops there: a provider that keeps
    /// sending would otherwise be read for as long as it kept sending — a
    /// Container's chunks are refused as soon as the sequence overruns
    /// (spec: FM-5), but refusing them does not end the transfer the way running
    /// out of stream does. The one byte past is what leaves a caller counting
    /// what arrived against [`len`](Self::len) able to tell an answer that ran
    /// long from one that was exact.
    pub fn into_reader(self) -> Pin<Box<dyn AsyncRead + Send + 'static>> {
        let declared = self.len;
        Box::pin(self.reader.take(declared.saturating_add(1)))
    }

    /// Drains the stream into memory, checking it was as long as it claimed.
    ///
    /// Only for streams whose length is the *caller's* own number: a body this
    /// device is sending, or a fixture it just built. Everything that arrives
    /// from Storage is sized by something outside the trust boundary — a
    /// provider's `Content-Length`, or a length inside an object anyone could
    /// have edited — and takes [`into_bytes_within`](Self::into_bytes_within) or
    /// [`collect_exact`](Self::collect_exact), each of which says how many bytes
    /// it is willing to spend before the first one arrives.
    ///
    /// A gateway handing the bytes on, and any reader that must not size its
    /// memory by the object at all, go through [`ByteStream::into_reader`]: a
    /// fetch decodes a Container chunk by chunk onto disk and never holds one,
    /// whatever a Pack weighs.
    pub async fn into_bytes(self) -> Result<Vec<u8>> {
        let expected = self.len;
        self.collect_exact(expected).await
    }

    /// The same drain, refusing a stream that declares more than `ceiling`
    /// before a byte of it is read.
    ///
    /// For an answer whose size is the provider's to state and whose
    /// *legitimate* size the caller knows: a control object is read whole, and
    /// how large one of its kind may be is a format ceiling; a gateway reading
    /// one of the provider's own documents brings a ceiling of its own instead,
    /// since no schema of this Library's covers those. A provider — or an
    /// account somebody else has written into — that declares more than that
    /// has declared something no answer to this call could be, and the refusal
    /// costs nothing, because nothing was allocated for the claim.
    pub async fn into_bytes_within(self, ceiling: u64) -> Result<Vec<u8>> {
        let declared = self.len;
        if declared > ceiling {
            return Err(Error::ObjectTooLarge { declared, ceiling });
        }
        self.collect_exact(declared).await
    }

    /// The first `len` bytes of the answer, however many the provider sends.
    ///
    /// For a read that wants a front rather than a whole: a control object's
    /// 44-byte header, which is all the two questions asked before a commit
    /// need, or as much of a refusal as could carry the reason for it. A
    /// provider that ignored the range and started sending the whole object is
    /// harmless here and stays harmless: the front is parsed off whatever
    /// arrives, and the rest is neither buffered nor waited for. A short answer
    /// comes back short, for the parse to refuse as the header it is not.
    ///
    /// `len` bounds the reading and not the allocation: a front stated in
    /// megabytes — because that is the width past which an answer is not the
    /// document it was asked for — still starts at the same reserve a whole
    /// drain does, and grows only as bytes arrive.
    pub async fn collect_front(self, len: u64) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(usize::try_from(len.min(RESERVE)).unwrap_or(0));
        self.reader.take(len).read_to_end(&mut bytes).await?;
        Ok(bytes)
    }

    /// The same drain, held against the caller's own count rather than against
    /// the stream's claim.
    ///
    /// A ranged read already knows how many bytes it asked Storage for, and that
    /// is the stronger number to check: a provider that answered with some other
    /// part of the object — or with the whole of it, having ignored the range —
    /// is caught here rather than left to look like a Container that will not
    /// open.
    ///
    /// The count bounds the reading as well as judging it. An answer that runs
    /// past `expected` stops one byte past it rather than growing to whatever
    /// the provider felt like sending, so the refusal costs the same whether the
    /// overrun is one byte or a gigabyte; an answer that stops short is refused
    /// for what it is.
    pub async fn collect_exact(self, expected: u64) -> Result<Vec<u8>> {
        let mut bytes = Vec::with_capacity(usize::try_from(expected.min(RESERVE)).unwrap_or(0));
        // One byte past what was asked for: enough to tell "exactly this" from
        // "more than this", and never more than that.
        let mut reader = self.reader.take(expected.saturating_add(1));
        reader.read_to_end(&mut bytes).await?;

        let actual = bytes.len() as u64;
        if actual > expected {
            return Err(Error::LengthOverrun { expected });
        }
        if actual < expected {
            return Err(Error::LengthMismatch { expected, actual });
        }
        Ok(bytes)
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
        let result = stream.into_bytes().await;
        assert!(
            matches!(
                result,
                Err(Error::LengthMismatch {
                    expected: 64,
                    actual: 10,
                })
            ),
            "expected 64 bytes and only 10 to arrive, got {result:?}"
        );
    }

    #[tokio::test]
    async fn more_than_a_ranged_read_asked_for_is_caught_too() {
        let stream = ByteStream::from(b"the whole object".to_vec());
        let result = stream.collect_exact(4).await;
        assert!(
            matches!(result, Err(Error::LengthOverrun { expected: 4 })),
            "expected 4 bytes and more to arrive, got {result:?}"
        );
    }

    /// A reader that would hand over far more than anyone asked for.
    ///
    /// Not a length in a header this time but the answer itself: a provider —
    /// or something between one and this device — that keeps sending. What is
    /// being checked is that the reading stops at the bound rather than growing
    /// to whatever is offered, so the source is deliberately larger than any
    /// bound a case here states.
    fn a_flood(bytes: usize) -> std::io::Cursor<Vec<u8>> {
        std::io::Cursor::new(vec![0x5a; bytes])
    }

    // The overrun is refused without the excess being buffered: the read stops
    // one byte past what was asked for, whether the answer is one byte too long
    // or a thousand.
    #[tokio::test]
    async fn an_overrunning_answer_is_stopped_at_the_bound() {
        let stream = ByteStream::new(8, a_flood(64 * 1024));
        let result = stream.into_bytes().await;
        assert!(
            matches!(result, Err(Error::LengthOverrun { expected: 8 })),
            "expected a stream declaring 8 bytes to stop at 8, got {result:?}"
        );
    }

    // A declared length past what the caller will spend is answered before the
    // reader is touched at all — which is what makes it cheap: the stream here
    // claims four gigabytes and nothing is allocated for the claim.
    #[tokio::test]
    async fn a_declaration_past_the_ceiling_is_refused_before_reading() {
        let stream = ByteStream::new(u64::from(u32::MAX), a_flood(64 * 1024));
        let result = stream.into_bytes_within(64 * 1024).await;
        assert!(
            matches!(
                result,
                Err(Error::ObjectTooLarge {
                    declared,
                    ceiling: 65_536,
                }) if declared == u64::from(u32::MAX)
            ),
            "expected a four-gigabyte claim to be refused outright, got {result:?}"
        );
    }

    // Inside the ceiling it is the ordinary drain, and the declared length is
    // still what the answer is held against.
    #[tokio::test]
    async fn a_stream_inside_the_ceiling_is_drained_and_still_checked() {
        let stream = ByteStream::from(b"a control object's bytes".to_vec());
        assert_eq!(
            stream.into_bytes_within(1024).await.unwrap(),
            b"a control object's bytes",
        );

        let short = ByteStream::new(64, std::io::Cursor::new(b"only ten b".to_vec()));
        let result = short.into_bytes_within(1024).await;
        assert!(
            matches!(
                result,
                Err(Error::LengthMismatch {
                    expected: 64,
                    actual: 10,
                })
            ),
            "expected a stream inside the ceiling to be held against its own claim, got {result:?}"
        );
    }

    // A fixed-size front takes what it came for and leaves the rest, however
    // much of the object the provider decided to send.
    #[tokio::test]
    async fn a_front_takes_its_own_length_and_no_more() {
        let stream = ByteStream::new(44, a_flood(64 * 1024));
        let front = stream
            .collect_front(44)
            .await
            .expect("a front is read off whatever arrives");
        assert_eq!(front.len(), 44);

        // And a provider that answered short hands back what it had, for the
        // parse to refuse as the header it is not.
        let short = ByteStream::new(44, std::io::Cursor::new(vec![0x5a; 10]));
        assert_eq!(short.collect_front(44).await.unwrap().len(), 10);
    }

    // A wide front is a bound on the reading and not a budget to hold open. The
    // fronts a caller states can be a megabyte, and the ordinary answer is a few
    // hundred bytes, so allocating the width would spend the ceiling on every
    // one of them.
    #[tokio::test]
    async fn a_wide_front_is_not_allocated_in_advance_of_the_bytes() {
        const WIDE: u64 = 1024 * 1024;
        let stream = ByteStream::new(WIDE, std::io::Cursor::new(vec![0x5a; 300]));
        let front = stream
            .collect_front(WIDE)
            .await
            .expect("a front is read off whatever arrives");

        assert_eq!(front.len(), 300);
        assert!(
            (front.capacity() as u64) <= RESERVE,
            "a 300-byte answer to a megabyte-wide front must not have cost a \
             megabyte, held {} bytes of capacity",
            front.capacity(),
        );
    }

    // A caller that streams the answer past a decoder instead of holding it is
    // bounded by the declaration too. Nothing here grows with the flood — that
    // is the point of streaming — but a read that followed one would never come
    // back, and the fetch paths draining a Container are exactly that read.
    #[tokio::test]
    async fn a_streamed_answer_stops_one_byte_past_its_declaration() {
        let stream = ByteStream::new(8, a_flood(64 * 1024));
        let mut reader = stream.into_reader();

        let mut drained = Vec::new();
        reader
            .read_to_end(&mut drained)
            .await
            .expect("a bounded reader ends");
        assert_eq!(
            drained.len(),
            9,
            "the eight bytes declared, and the one that says there were more",
        );
    }
}
