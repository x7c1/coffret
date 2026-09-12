use std::path::Path;

use crate::folder_entry::FolderEntry;
use crate::local_io_error::LocalIoError;
use crate::mapped_relative_location::MappedRelativeLocation;
use crate::root_probe::RootProbe;
use crate::source_reader::SourceReader;
use async_trait::async_trait;

/// Everything the flows ask of the folders this device maps into the Library.
///
/// The reading half of what [`Spool`](crate::Spool) is the writing half of, and
/// a capability for the same reason: what a sync and a freeze promise around a
/// mapped folder are promises about *failure* and about *absence* — a root that
/// is not there says nothing about the Library rather than saying every Entry
/// under it is gone (spec: EP-12), a subfolder that vanished mid-walk holds no
/// more files, a listing that is refused fails the run — and a real filesystem
/// that cannot be asked to refuse a chosen step leaves all of them untested.
///
/// What is deliberately *not* here is every decision about what the walk means.
/// Which folders to descend into, how a name becomes an Entry Path (spec: EP-1,
/// EP-2), what two files claiming one path is, and what an identity that moved
/// says about a root — all of that is the use case's, and stays there. What
/// moves behind this line is only how the operating system is asked: stat the
/// root following links, list one folder without following them, open a regular
/// file, and the platform's own spelling of a filesystem's identity.
///
/// Every operation fails with [`LocalIoError`], and the contract's other half is
/// not in the signatures: **absence is an [`Option`] and never an error**. A
/// root that is not there and a folder that is not there are ordinary outcomes
/// this walk has verdicts for, so no part of the use-case layer reads an
/// [`io::ErrorKind`](std::io::ErrorKind) to find out which it was — the gateway
/// swallows it, exactly as it swallows the absence a
/// [`discard`](crate::Spool::discard) tolerates (spec: OC-8).
///
/// The trait is object safe, so a flow holds `&dyn MappedRoots` and is written
/// once against the device's own folders and against the in-memory fake alike.
#[async_trait]
pub trait MappedRoots: Send + Sync {
    /// States one mapped root, *following* links (spec: EP-12).
    ///
    /// Following them because that is what listing the folder would resolve
    /// anyway, and unlike the entries below it, which are stated with links
    /// unfollowed (spec: EP-8). A mapped root reached through a link is a folder
    /// the person pointed this device at on purpose.
    ///
    /// `Ok(None)` is the root not being there — the answer a whole rule rests
    /// on, because reading it as a folder holding nothing would report every
    /// Entry under it as deleted. `Ok(Some(_))` carries what the platform can
    /// say about the filesystem it stands on, which is what tells an unmounted
    /// mount point from a folder somebody emptied.
    ///
    /// Only absence is a verdict, and a root this process may not stat at all is
    /// a refusal like any other. A root that is a regular file is neither: it is
    /// there to be stated, so it comes back `Ok(Some(_))` like any other path
    /// that exists, and the run fails at the listing below — the call a mapped
    /// root that is not a folder actually refuses.
    async fn probe_root(&self, root: &Path) -> Result<Option<RootProbe>, LocalIoError>;

    /// The children of one folder below `root`, each stated *without* following
    /// links (spec: EP-8).
    ///
    /// `root` is the configured mapping and may itself resolve through a link.
    /// `relative` is the validated, spelling-preserving location below it. Each
    /// of those components is descended from the open root without following a
    /// link, so a parent replaced since an earlier enumeration cannot redirect
    /// this listing.
    ///
    /// Stated here rather than by the caller, because the listing and the stat
    /// are one question about one moment: a name read now and stated later is a
    /// second look at a folder that may have moved in between. A child that went
    /// away between the two is left out, for the same reason a folder that went
    /// away is `Ok(None)` — it holds nothing to carry into the Library.
    ///
    /// Two calls therefore stand behind one answer, and a refusal says which of
    /// them it was: [`Listing`](crate::LocalOperation::Listing) for this folder,
    /// carrying this folder's path, and
    /// [`Stating`](crate::LocalOperation::Stating) for one child, carrying *that
    /// child's* path. A caller that puts the path in front of a person has to
    /// read the operation to know what sentence belongs beside it — reporting a
    /// child's path under a sentence about the folder sends them to look at the
    /// wrong thing.
    ///
    /// `Ok(None)` is the folder not being there, and stands for nothing else: a
    /// subfolder that vanished mid-walk, or a root that went between the probe
    /// and the listing. Something that *is* at the path and is not a folder — a
    /// mapped root somebody pointed at a file — is a refusal instead, because
    /// answering it with nothing would reach the walk as the missing-root
    /// verdict and quietly stop backing the mapping up (spec: EP-12).
    ///
    /// The order is the filesystem's own and means nothing; the walk keys what
    /// it finds by Entry Path (spec: EP-3).
    async fn list_folder(
        &self,
        root: &Path,
        relative: Option<&MappedRelativeLocation>,
    ) -> Result<Option<Vec<FolderEntry>>, LocalIoError>;

    /// Opens one regular file below `root` for streaming reads.
    ///
    /// The root is deliberately resolved as configured; every component in the
    /// validated relative location, including the final filename, is opened
    /// without following links. Nonregular final names are refused without a
    /// potentially blocking read.
    ///
    /// A missing file is a refusal here and not an [`Option`]: this capability
    /// opens a specific source after its caller has decided that it should exist.
    /// Callers for which disappearance is an ordinary outcome interpret that
    /// refusal at their own boundary; it is never a verdict about a folder.
    async fn open_source(
        &self,
        root: &Path,
        relative: &MappedRelativeLocation,
    ) -> Result<Box<dyn SourceReader>, LocalIoError>;
}
