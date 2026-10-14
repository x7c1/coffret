use std::ffi::OsString;
use std::path::Path;

use async_trait::async_trait;
use tokio::io::AsyncRead;

use crate::local_io_error::LocalIoError;
use crate::spool_writer::SpoolWriter;

/// Everything a fetch asks of the place the parcels it read are kept.
///
/// A fetched parcel stays on this device until every Entry it covers that the
/// device maps is on disk or witnessed absent, and a parcel the device holds is
/// never requested from Storage again (spec: PK-21). Its ciphertext needs a
/// home for that, and it is this device's own disk — a directory beside the
/// spool, under the state directory — so nothing here crosses the trust
/// boundary [`ObjectStore`](crate::ObjectStore) exists to cross: what is kept
/// is the parcel exactly as Storage sent it, still sealed under the Container
/// Key, and every chunk of it authenticates again whenever it is read back
/// (spec: FM-5, FM-8).
///
/// It is a capability rather than calls on a filesystem for the reason
/// [`Spool`](crate::Spool) is one, whose shape it takes: what a fetch promises
/// about a kept parcel is a promise about *absence* and *interruption* — a
/// parcel file that is gone or that does not authenticate is not held, and is
/// fetched again with a finding (spec: PK-21) — and a filesystem that cannot be
/// made to lose a file between two calls leaves that untested.
///
/// The [`Index`](crate::Index) records which parcels are held, and that record
/// is the handle on what is in the directory: a fetch reads a parcel by the row
/// naming it and never by finding its file. The row is written before the file
/// is created, the precedent the spool sets (spec: OC-2), and a row whose file
/// is short or missing is the case the fetch already meets as "not held".
///
/// What row-first order cannot rule out is a file no row names, which a
/// letting go that runs beside a fetch can leave. The directory is
/// [`list`](Self::list)ed only to discard those, by
/// [`sweep_unnamed_parcels`](crate::fetch::sweep_unnamed_parcels) before a
/// server serves anything (spec: PK-21).
///
/// **A file that is already gone is a successful [`discard`](Self::discard)**,
/// and a file that is not there is `None` from [`reader`](Self::reader) rather
/// than a failure: letting a parcel go is run again after an interruption
/// (spec: OC-8), and no part of the use-case layer reads an
/// [`io::ErrorKind`](std::io::ErrorKind) to find out which it was.
#[async_trait]
pub trait ParcelFiles: Send + Sync {
    /// Makes the parcel directory, and the folders above it, where they are
    /// missing. One that is already there is success, the way `mkdir -p` is.
    async fn prepare_dir(&self, dir: &Path) -> Result<(), LocalIoError>;

    /// Opens a parcel file for writing, replacing anything at that path.
    ///
    /// The writer is the one a spool file is written through: what a kept
    /// parcel needs of it is the same promise, that a finished file is on the
    /// device rather than only handed to the operating system.
    async fn create(&self, path: &Path) -> Result<Box<dyn SpoolWriter>, LocalIoError>;

    /// A reader over a kept parcel, or `None` where no file is at the path.
    ///
    /// A reader rather than the bytes: a parcel is tens of megabytes
    /// (spec: PK-19), and what is read back goes through the chunk decoder a
    /// transfer buffer at a time, exactly as a parcel arriving from Storage
    /// does.
    async fn reader(
        &self,
        path: &Path,
    ) -> Result<Option<Box<dyn AsyncRead + Send + Unpin>>, LocalIoError>;

    /// The names of the files in the parcel directory, in no particular order.
    ///
    /// A directory that is not there lists nothing: a device that has never
    /// kept a parcel has not made it.
    async fn list(&self, dir: &Path) -> Result<Vec<OsString>, LocalIoError>;

    /// How long the parcel file at `path` is, or `None` where no file is there.
    async fn size(&self, path: &Path) -> Result<Option<u64>, LocalIoError>;

    /// Removes one parcel file, the parcel having been let go.
    ///
    /// A file that is already gone is the same outcome as one this call
    /// removed (spec: OC-8).
    async fn discard(&self, path: &Path) -> Result<(), LocalIoError>;
}
