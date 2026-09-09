use async_trait::async_trait;

use crate::local_io_error::LocalIoError;

/// One local file open for reading, handing its plaintext over a buffer at a
/// time.
///
/// Handed out by [`MappedRoots::open_source`](crate::MappedRoots::open_source).
/// A reader rather than the bytes, because a Pack's members are walked twice —
/// hashed before the entry table is written and fed through the encoder
/// afterwards — and neither pass may be bounded by what fits in memory
/// (spec: FM-2, FM-5, PK-5). The step that can afford the whole file builds it
/// from this same reader.
///
/// It is not [`Sync`]: one reader belongs to the step reading one file, and
/// nothing shares it.
#[async_trait]
pub trait SourceReader: Send {
    /// The length reported by the handle when it was opened.
    ///
    /// This belongs to the reader rather than to a separate path stat: the
    /// name may be replaced after the open, while the bytes this handle yields
    /// remain the originally opened file's.
    fn len(&self) -> u64;

    /// Whether the opened file is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Fills `buffer` with the next stretch of the file.
    ///
    /// Zero means the file is exhausted, which is the only way a caller learns
    /// how long the file turned out to be — a length the scan's stat may no
    /// longer agree with. A short fill is not the end: only zero is.
    async fn read(&mut self, buffer: &mut [u8]) -> Result<usize, LocalIoError>;
}
