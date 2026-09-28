use crate::error::{Error, Result};

/// How many bytes a Storage answer has delivered, held to the two lengths it
/// has to keep to.
///
/// One is the length the answer declared, which the stream is read no further
/// than one byte past (see [`ByteStream::into_reader`]). The other is the
/// length the caller knows the answer has to be: what a range read asked for,
/// or what the catalog records for the object (spec: FM-15). An answer that
/// breaks either is Storage's doing, so both refusals are the port's —
/// [`Error::LengthOverrun`] and [`Error::LengthMismatch`] — and this is the one
/// place they are decided, for the drain that holds a whole answer and the
/// fetches that stream one past a decoder alike.
///
/// A byte past the bound is refused as it arrives, before whatever reads the
/// answer is handed it; how much more there was is nothing this device pays to
/// find out. Everything else can only be decided once the answer has ended,
/// which is [`finish`](Self::finish).
///
/// [`ByteStream::into_reader`]: crate::ByteStream::into_reader
#[derive(Debug)]
pub(crate) struct AnswerLength {
    declared: u64,
    bound: u64,
    received: u64,
}

impl AnswerLength {
    /// Starts counting an answer that declared `declared` bytes and has to be
    /// `bound` bytes long.
    pub(crate) fn new(declared: u64, bound: u64) -> Self {
        Self {
            declared,
            bound,
            received: 0,
        }
    }

    /// Counts `read` more bytes, refusing the first one past the bound.
    pub(crate) fn count(&mut self, read: usize) -> Result<()> {
        self.received = self.received.saturating_add(read as u64);
        if self.received > self.bound {
            return Err(Error::LengthOverrun {
                expected: self.bound,
            });
        }
        Ok(())
    }

    /// Decides the answer that has ended: exactly its declaration, and exactly
    /// the bound.
    ///
    /// The declaration first. A short answer is known exactly and a long one
    /// only to be long, because the reader stopped one byte past the
    /// declaration rather than following it. An answer that kept to its own
    /// declaration and declared less than the bound is then Storage answering
    /// short — which, for a whole object, a hash mismatch would otherwise report
    /// as the object being wrong.
    pub(crate) fn finish(self) -> Result<()> {
        let Self {
            declared,
            bound,
            received,
        } = self;
        if received > declared {
            return Err(Error::LengthOverrun { expected: declared });
        }
        if received < declared {
            return Err(Error::LengthMismatch {
                expected: declared,
                actual: received,
            });
        }
        if received < bound {
            return Err(Error::LengthMismatch {
                expected: bound,
                actual: received,
            });
        }
        Ok(())
    }
}
