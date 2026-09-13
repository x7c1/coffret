use coffret_usecase::ByteStream;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::http::expected_answer::ExpectedAnswer;
use crate::http::transport_error::TransportError;

/// Turns what arrived into the stream the port hands on, or refuses it.
///
/// The port's streams know how long they are, so an answer that declares a
/// length is already one and is handed on: what holds it is the reckoning of
/// whoever drains it — the format's ceiling for a control object, the gateway's
/// own for a document — one layer up from here.
///
/// An answer that declares none is the case this function exists for, and what
/// it costs depends entirely on what the call asked for: a document is collected
/// against the ceiling the caller brought, and a Storage Object's bytes are
/// refused — as the undeclared length they are, rather than against a number
/// meant for JSON. Why each is that way is on
/// [`ExpectedAnswer`](crate::http::ExpectedAnswer).
pub(crate) async fn answer_body(
    expected: ExpectedAnswer,
    status: u16,
    declared: Option<u64>,
    reader: impl AsyncRead + Send + 'static,
) -> Result<ByteStream, TransportError> {
    match (declared, carried(expected, status)) {
        (Some(len), _) => Ok(ByteStream::new(len, reader)),
        (None, ExpectedAnswer::Document { within }) => collect_within(reader, within).await,
        (None, ExpectedAnswer::ObjectBytes) => Err(TransportError::UndeclaredObjectLength),
    }
}

/// What the answer actually carries, which is what the call asked for only where
/// the call was answered.
///
/// A refusal is one of Drive's error envelopes whatever was asked for: a `get`
/// answered 404 carries a reason and not an object's bytes, and reading that
/// reason is how a missing object becomes
/// [`NotFound`](coffret_usecase::Error::NotFound) rather than a transport
/// failure. So the status settles the shape before the expectation does, and a
/// refusal that declares no length is collected like every other document.
fn carried(expected: ExpectedAnswer, status: u16) -> ExpectedAnswer {
    match expected {
        ExpectedAnswer::ObjectBytes if !(200..300).contains(&status) => ExpectedAnswer::DOCUMENT,
        expected => expected,
    }
}

/// Drains an answer that declared no length, up to what the call will take.
///
/// The bound is the caller's number, so the ceiling belongs to the document the
/// request asked for rather than to this module. Reading stops one byte past it
/// — the least it takes to tell "the whole answer" from "more than the caller
/// asked for" — so a body that never ends costs the ceiling and not the machine.
async fn collect_within(
    reader: impl AsyncRead + Send + 'static,
    ceiling: u64,
) -> Result<ByteStream, TransportError> {
    let mut reader = Box::pin(reader).take(ceiling.saturating_add(1));
    let mut collected = Vec::new();
    reader
        .read_to_end(&mut collected)
        .await
        .map_err(|cause| TransportError::Body {
            detail: cause.to_string(),
        })?;

    if collected.len() as u64 > ceiling {
        return Err(TransportError::AnswerTooLong { ceiling });
    }
    Ok(ByteStream::from(collected))
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;
    use std::task::{Context, Poll};

    use tokio::io::ReadBuf;

    use super::*;
    use crate::answer_ceiling::MAX_DOCUMENT_LEN;

    /// A body that never ends, counting what it was asked to give.
    ///
    /// A reader that stops where it said it would takes a bounded number of
    /// bytes off this and returns; one that grows to whatever arrives never
    /// returns at all. The count is what lets a case state the cost rather than
    /// trust it.
    struct Endless {
        handed: Arc<AtomicU64>,
    }

    impl AsyncRead for Endless {
        fn poll_read(
            self: Pin<&mut Self>,
            _context: &mut Context<'_>,
            buf: &mut ReadBuf<'_>,
        ) -> Poll<io::Result<()>> {
            let take = buf.remaining();
            buf.initialize_unfilled_to(take);
            buf.advance(take);
            self.handed.fetch_add(take as u64, Ordering::Relaxed);
            Poll::Ready(Ok(()))
        }
    }

    /// An endless body and the counter of what has been taken off it.
    fn endless() -> (Endless, Arc<AtomicU64>) {
        let handed = Arc::new(AtomicU64::new(0));
        (
            Endless {
                handed: Arc::clone(&handed),
            },
            handed,
        )
    }

    // The whole point of the shape being on the request: an object fetch that
    // Drive answers without a `Content-Length` — chunked transfer encoding is
    // the ordinary way that happens — is not a document, and 1 MiB is not a
    // number a Container is measured against. A Pack of photographs is past it
    // almost always, so holding one to it would refuse nearly every fetch in a
    // real Library.
    #[tokio::test]
    async fn an_object_answered_without_a_length_is_not_held_to_the_document_ceiling() {
        let (body, handed) = endless();

        let refusal = answer_body(ExpectedAnswer::ObjectBytes, 200, None, body)
            .await
            .err();

        assert!(
            matches!(refusal, Some(TransportError::UndeclaredObjectLength)),
            "an object's bytes must be refused for the length they did not declare, \
             got {refusal:?}",
        );
        assert!(
            !matches!(refusal, Some(TransportError::AnswerTooLong { .. })),
            "and never for passing a ceiling meant for JSON",
        );
        assert_eq!(
            handed.load(Ordering::Relaxed),
            0,
            "refusing it cost nothing to read: not a megabyte, and not a Container",
        );
    }

    // And the sentence a caller gets says what was measured and what it was
    // measured against, so that neither refusal reads as the other, and neither
    // reads as a network that dropped.
    #[tokio::test]
    async fn each_refusal_says_what_it_was_measured_against() {
        let object = TransportError::UndeclaredObjectLength.to_string();
        assert!(
            object.contains("declared no length"),
            "an object refused for its missing length must say so: {object}",
        );

        let document = TransportError::AnswerTooLong {
            ceiling: MAX_DOCUMENT_LEN,
        }
        .to_string();
        assert!(
            document.contains(&MAX_DOCUMENT_LEN.to_string()),
            "a document refused for its size must name the ceiling it passed: {document}",
        );

        let broken = TransportError::Body {
            detail: "connection reset".to_owned(),
        }
        .to_string();
        assert_ne!(
            object, broken,
            "and neither refusal may read as a connection that broke",
        );
        assert_ne!(document, broken);
    }

    // A document is still collected when it declares no length, which is what
    // the ceiling is there to bound — and the bound is the caller's number, not
    // this module's.
    #[tokio::test]
    async fn a_document_that_declares_no_length_is_collected_up_to_its_ceiling() {
        let body = io::Cursor::new(vec![0x7b; 512]);

        let collected = answer_body(ExpectedAnswer::DOCUMENT, 200, None, body)
            .await
            .expect("a document inside its ceiling is collected");

        assert_eq!(collected.len(), 512);
    }

    // Past it, the refusal names the ceiling, and reading stops one byte over it
    // rather than following a body that never ends.
    #[tokio::test]
    async fn a_document_past_its_ceiling_is_refused_against_the_ceiling_it_passed() {
        let (body, handed) = endless();
        let ceiling = 4096;

        let refusal = answer_body(
            ExpectedAnswer::Document { within: ceiling },
            200,
            None,
            body,
        )
        .await
        .err();

        assert!(
            matches!(refusal, Some(TransportError::AnswerTooLong { ceiling: stated }) if stated == ceiling),
            "expected a refusal naming the {ceiling} bytes the call takes in, got {refusal:?}",
        );
        assert_eq!(
            handed.load(Ordering::Relaxed),
            ceiling + 1,
            "an endless answer cost one byte more than the ceiling, and no more",
        );
    }

    // A refusal to an object fetch is an error envelope and not an object, so it
    // is read as the document it is. Without this, a `get` of an object Drive
    // answers 404 for — and answers without a length — would come back as a
    // transport failure, and a missing object would stop being `NotFound`.
    #[tokio::test]
    async fn a_refusal_to_an_object_fetch_is_read_as_the_document_it_is() {
        let envelope = br#"{"error":{"message":"File not found"}}"#;
        let body = io::Cursor::new(envelope.to_vec());

        let collected = answer_body(ExpectedAnswer::ObjectBytes, 404, None, body)
            .await
            .expect("an error envelope is a document however it was asked for");

        assert_eq!(collected.len(), envelope.len() as u64);
    }

    // An answer that declares a length is handed on as a stream whatever shape
    // was asked for: nothing here holds it, because the reckoning that does is
    // the caller's and happens one layer up.
    #[tokio::test]
    async fn an_answer_that_declares_a_length_is_handed_on_whole() {
        let declared = 8 * 1024 * 1024;
        let (body, handed) = endless();

        let stream = answer_body(ExpectedAnswer::ObjectBytes, 200, Some(declared), body)
            .await
            .expect("a declared length is the port's own stream");

        assert_eq!(stream.len(), declared, "past the document ceiling and kept");
        assert_eq!(
            handed.load(Ordering::Relaxed),
            0,
            "and not a byte of it collected on the way through",
        );
    }
}
