use std::path::Path;

use async_trait::async_trait;
use tokio::io::AsyncRead;

use crate::local_io_error::LocalIoError;
use crate::spool_writer::SpoolWriter;

/// Everything the flows ask of the place a Container waits before it is
/// uploaded.
///
/// The spool is this device's own disk and not Storage, so nothing here crosses
/// the trust boundary the [`ObjectStore`](crate::ObjectStore) port exists to
/// cross. It is a capability all the same, for a different reason: the promises
/// a sync and a freeze make around a spool are promises about *failure* — the
/// pending row is written before the file can exist (spec: OC-2), the row is
/// flipped to [`Spooled`](crate::device_state::SpoolState::Spooled) only once
/// the bytes are on the device, and an abandoned spool is disposed of however
/// far its writing got (spec: OC-6) — and a filesystem that cannot be made to
/// fail at a chosen step leaves every one of them untested.
///
/// The four operations are the whole of what the spool lifecycle needs:
/// [`prepare_dir`](Self::prepare_dir) once per run,
/// [`create`](Self::create) and the [`SpoolWriter`] it answers with per
/// Container, [`open`](Self::open) to stream a finished one to Storage, and
/// [`discard`](Self::discard) when its Container is committed or abandoned.
/// Nothing lists the directory: the pending rows are the only handle on what is
/// in it, which is what the ordering above is for.
///
/// Every operation fails with [`LocalIoError`], and one rule of the contract is
/// not in the signatures: **a file that is already gone is a successful
/// [`discard`](Self::discard)**. Absence is an ordinary outcome and not only a
/// repeated one — a row may name a file whose creation never happened — so the
/// caller simply calls `discard`, and no part of the use-case layer reads an
/// [`io::ErrorKind`](std::io::ErrorKind) to find out which it was.
///
/// The trait is object safe, so a flow holds `&dyn Spool` and is written once
/// against the device's disk and against the in-memory fake alike.
#[async_trait]
pub trait Spool: Send + Sync {
    /// Makes the spool directory, and the folders above it, where they are
    /// missing.
    ///
    /// A run calls it before it spools anything and never again: the directory
    /// is where a whole batch waits. One that is already there is success, the
    /// way `mkdir -p` is.
    async fn prepare_dir(&self, dir: &Path) -> Result<(), LocalIoError>;

    /// Opens a spool file for writing, replacing anything at that path.
    ///
    /// The pending row naming the path is already recorded when this is called
    /// (spec: OC-2), so a failure here leaves a row naming a file that never
    /// came to exist — a state disposal tolerates rather than one it has to be
    /// spared.
    async fn create(&self, path: &Path) -> Result<Box<dyn SpoolWriter>, LocalIoError>;

    /// Opens a finished spool for reading, to stream it to Storage.
    ///
    /// A reader rather than the bytes: a Pack is larger than memory
    /// (spec: PK-5), so what an upload takes from here goes straight into a
    /// [`ByteStream`](crate::ByteStream). Each attempt of a retried upload asks
    /// for a fresh one, because the stream is consumed by the attempt that
    /// failed.
    async fn open(&self, path: &Path) -> Result<Box<dyn AsyncRead + Send + Unpin>, LocalIoError>;

    /// Removes one spool file, its Container having been committed or
    /// abandoned.
    ///
    /// A file that is already gone is the same outcome as one this call
    /// removed, so an interrupted cleanup is simply run again (spec: OC-6). See
    /// the trait for why that tolerance is the contract's and not the caller's.
    async fn discard(&self, path: &Path) -> Result<(), LocalIoError>;
}
