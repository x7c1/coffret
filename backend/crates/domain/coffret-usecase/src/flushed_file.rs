use std::time::SystemTime;

use async_trait::async_trait;

use crate::below_root_error::BelowRootError;

/// One scratch whose bytes are on the device, waiting to be given its
/// final name.
///
/// What [`ScratchFile::flush`](crate::ScratchFile::flush) hands back, and the
/// second half of the type transition that orders EP-11's steps: nothing can be
/// published before it has been flushed, and nothing can be written after.
///
/// It is not [`Sync`]: one of these belongs to the step placing one Entry.
#[async_trait]
pub trait FlushedFile: Send {
    /// Sets the file's modification time to the one its Entry carries
    /// (spec: FM-9).
    ///
    /// Handed a moment this platform's clock already reaches, rather than the
    /// Entry's own [`Mtime`](coffret_model::Mtime): whether an Entry's time can
    /// be set on this device at all is a question about the Entry, and the flow
    /// that places it asks it and names the refusal
    /// ([`Mtime::to_system_time`](coffret_model::Mtime::to_system_time)) before
    /// the file is handed over. What is left for a filesystem to refuse is its
    /// own business.
    ///
    /// Set before the rename, so the file that appears at the final path is
    /// already stamped: a scan that ran between the two would otherwise see a
    /// file whose time is neither the Entry's nor anything the device wrote down.
    /// Set on the handle this run has been writing to rather than by opening the
    /// name again, which is the only form that keeps the confinement — reopening
    /// by path would be the one place a symbolic link could get between the
    /// descent and the stamp.
    ///
    /// `async` because it is whatever the gateway needs it to be: setting times
    /// on an open handle is a blocking metadata call on a real filesystem and
    /// nothing at all in a fake, and the caller may not care which.
    ///
    /// # Errors
    ///
    /// [`BelowRootError::Io`] carrying
    /// [`Stamping`](crate::LocalOperation::Stamping), where the operating
    /// system refused to set the time.
    async fn stamp(&mut self, modified: SystemTime) -> Result<(), BelowRootError>;

    /// Renames the file onto the destination's final name, which is the moment
    /// it exists.
    ///
    /// Within the folder the descent left open, and onto the name that descent
    /// was given, so the file lands where the walk arrived whatever has happened
    /// to the path above it since. A rename within one directory is atomic,
    /// which is what makes it the moment a reader can first see the file — whole,
    /// stamped, and verified, or not at all (spec: EP-11). Anything standing at
    /// that name is replaced, which is what a rename does.
    ///
    /// Synchronous, for the reason [`Destination::remove`](crate::Destination::remove)
    /// is: it is one call against a folder that is already open. It takes
    /// `self: Box<Self>`, so a published file is not one anybody still holds a
    /// handle to.
    fn publish(self: Box<Self>) -> Result<(), BelowRootError>;
}
