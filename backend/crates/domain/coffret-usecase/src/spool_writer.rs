use async_trait::async_trait;

use crate::local_io_error::LocalIoError;

/// One spool file open for writing.
///
/// Handed out by [`Spool::create`](crate::Spool::create), and consumed by
/// [`finish`](Self::finish): a Container may only be handed on once its
/// ciphertext is on the device, so the type makes "written" and "durable" two
/// different things and lets a caller hold the second only.
///
/// It is not [`Sync`]: one writer belongs to the step writing one Container,
/// and nothing shares it.
#[async_trait]
pub trait SpoolWriter: Send {
    /// Writes the next stretch of ciphertext.
    ///
    /// A short write is the implementation's to retry: this returns either all
    /// the bytes written or a failure.
    async fn write(&mut self, bytes: &[u8]) -> Result<(), LocalIoError>;

    /// Flushes the file to the device and closes it.
    ///
    /// To the device and not merely to the operating system, because the point
    /// of a spool is to still be there after the run that wrote it is not
    /// (spec: OC-2). It takes `self: Box<Self>` so that the writer is spent
    /// here: nothing may write to a spool a run has already called finished.
    async fn finish(self: Box<Self>) -> Result<(), LocalIoError>;
}
