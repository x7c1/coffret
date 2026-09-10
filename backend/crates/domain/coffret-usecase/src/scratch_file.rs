use async_trait::async_trait;

use crate::descent_error::DescentError;
use crate::flushed_file::FlushedFile;

/// One scratch inside a destination folder, open for writing.
///
/// Handed out by [`Destination::create`](crate::Destination::create), and
/// consumed by [`flush`](Self::flush): the type transition is the contract. A
/// file may not be published before it has been flushed, and may not be written
/// after — a crash that reordered the two would leave a name promising content
/// the device never wrote (spec: EP-11) — so "written" and "on the device" are
/// two different types and a caller holds the second only.
///
/// Both operations are `async`, because both are what a fetch spends its time
/// on: a Pack's Entry is written a transfer buffer at a time (spec: PK-5), and
/// the flush is a round trip to the device.
///
/// It is not [`Sync`]: one file belongs to the step placing one Entry, and
/// nothing shares it.
#[async_trait]
pub trait ScratchFile: Send {
    /// Writes the next stretch of the file's plaintext.
    ///
    /// A short write is the implementation's to retry: this returns either all
    /// the bytes written or a failure.
    async fn write(&mut self, bytes: &[u8]) -> Result<(), DescentError>;

    /// Flushes the file to the device and hands back what may be published.
    ///
    /// To the device and not merely to the operating system, because the rename
    /// behind it is what makes the file visible: a name that appeared before its
    /// content reached the disk would promise bytes a crash then lost
    /// (spec: EP-11). It takes `self: Box<Self>` so that the handle is spent
    /// here.
    async fn flush(self: Box<Self>) -> Result<Box<dyn FlushedFile>, DescentError>;
}
