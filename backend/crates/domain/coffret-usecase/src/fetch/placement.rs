use std::io;
use std::path::{Path, PathBuf};

use coffret_model::{ContentHash, EntryMetadata, EntryPath};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, warn};

use crate::device_state::{DeviceTime, LocalObservation};
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::target::Target;
use crate::index::Index;
use crate::local_mtime::system_time_of;
use crate::local_operation::LocalOperation;
use crate::scratch;

/// One Entry on its way into a mapped folder.
///
/// The order is what EP-11 asks for and the only order that gives it. The bytes
/// go into a temporary file beside their destination as they arrive, the file is
/// flushed, it is checked against the hash the current catalog records for the
/// Entry, it is stamped with the Entry's own modification time — and only then is
/// it renamed onto the final path. A rename within one directory is atomic, so
/// what a reader can see at the target path is either nothing or the whole
/// verified file: never a prefix of one, never one whose stamp has yet to be
/// set, and never one whose content has not been held against the catalog.
///
/// The three steps are three calls because the bytes arrive over a transfer
/// rather than all at once. [`write`](Self::write) takes whatever piece of the
/// Entry the last chunk carried, [`verify`](Self::verify) holds it against the
/// catalog, and [`publish`](Self::publish) is the moment the file exists.
/// Between the second and the third a caller can hold a whole Container's worth
/// of verified placements, which is what lets the object's own hash be checked
/// before any of them becomes visible (spec: FM-15, CP-11).
///
/// A placement that is not published has to be [`discard`](Self::discard)ed.
/// What must not be left behind is a half-written file inside a folder the sync
/// walks: the scratch prefix already keeps a scan from committing one (see
/// [`crate::scratch`]), and removing it keeps the folder from accumulating them.
pub(super) struct Placement<'a> {
    /// The Entry, and where on this device its file belongs.
    target: &'a Target,
    /// What the Container's own entry table records about it (spec: FM-9).
    entry: EntryMetadata,
    /// The temporary file, beside the destination, until it is closed.
    scratch_path: PathBuf,
    file: Option<fs::File>,
    /// The Entry's plaintext as it passes, for the check before the rename.
    hasher: blake3::Hasher,
    written: u64,
}

impl<'a> Placement<'a> {
    /// Opens a temporary file beside where the Entry's file will go.
    ///
    /// `entry` is the Container's own account of the Entry rather than the
    /// catalog's: it says how many bytes of the plaintext stream belong to this
    /// Entry, and holding the two accounts against each other is
    /// [`verify`](Self::verify)'s.
    pub(super) async fn open(target: &'a Target, entry: EntryMetadata) -> FetchResult<Self> {
        let directory = parent(&target.local_path)?;
        fs::create_dir_all(directory)
            .await
            .map_err(|cause| FetchError::Io {
                operation: LocalOperation::Creating,
                path: directory.to_path_buf(),
                cause,
            })?;

        let scratch_path = directory.join(scratch::name(target.location.container_id));
        let file = fs::File::create(&scratch_path)
            .await
            .map_err(refused(&scratch_path, LocalOperation::Creating))?;

        Ok(Self {
            target,
            entry,
            scratch_path,
            file: Some(file),
            hasher: blake3::Hasher::new(),
            written: 0,
        })
    }

    /// Where this Entry's plaintext starts in its Container's stream
    /// (spec: FM-9).
    pub(super) fn start(&self) -> u64 {
        self.entry.offset
    }

    /// Where it ends.
    pub(super) fn end(&self) -> u64 {
        self.entry.offset + self.entry.size
    }

    /// Takes the next piece of the Entry's plaintext.
    pub(super) async fn write(&mut self, bytes: &[u8]) -> FetchResult<()> {
        let file = self
            .file
            .as_mut()
            .expect("a placement is written to before it is verified");
        file.write_all(bytes)
            .await
            .map_err(refused(&self.scratch_path, LocalOperation::Writing))?;
        self.hasher.update(bytes);
        self.written += bytes.len() as u64;
        Ok(())
    }

    /// Closes the temporary file and holds it against the catalog.
    ///
    /// This is the check EP-11 makes the condition of a file becoming visible,
    /// and it is the second half of a pair. The chunks the bytes came out of
    /// authenticated, which proves they are a coffret object sealed under the
    /// key that opens this Container (spec: FM-5, FM-8). The hash the Index
    /// carries came out of the Journal record (spec: CP-11), so comparing the
    /// two is what proves the bytes are the *committed content this catalog
    /// stands for* rather than merely a well-formed Container.
    pub(super) async fn verify(&mut self) -> FetchResult<()> {
        let file = self
            .file
            .take()
            .expect("a placement is verified exactly once");
        // Flushed before the rename because the rename is what publishes the
        // file: a crash that reordered the two would leave a name promising
        // content the device never wrote.
        file.sync_all()
            .await
            .map_err(refused(&self.scratch_path, LocalOperation::Flushing))?;
        drop(file);

        let hash = ContentHash::from_bytes(*self.hasher.finalize().as_bytes());
        if self.written != self.entry.size || hash != self.target.location.entry.hash {
            return Err(FetchError::ContentMismatch {
                container_id: self.target.location.container_id,
                path: self.path().clone(),
            });
        }
        self.stamp().await
    }

    /// Renames the verified file onto its final path and records having placed
    /// it.
    ///
    /// The bookkeeping is not optional: fetching a file is exactly the second
    /// way a device materializes an Entry, so from here on the device may report
    /// the file as deleted if it goes missing, and the sync flow may offer a
    /// change to it back to the Library (spec: EP-10). What is recorded is the
    /// Entry's own size and modification time — which is what is now on disk —
    /// so the next scan and the next fetch both answer from the cheap comparison
    /// and open nothing.
    ///
    /// A rename that the operating system refuses takes the temporary file with
    /// it. This call consumes the placement, so no caller is left holding one to
    /// [`discard`](Self::discard), and what would otherwise stay behind is a
    /// scratch file inside a folder the sync walks. Once the rename has
    /// happened the file is the Entry's, and a bookkeeping failure after it
    /// leaves that file where it belongs rather than removing content this
    /// device verified.
    pub(super) async fn publish(
        self,
        index: &dyn Index,
        now: DeviceTime,
    ) -> FetchResult<EntryPath> {
        debug_assert!(
            self.file.is_none(),
            "a placement is verified before it is published",
        );
        if let Err(cause) = fs::rename(&self.scratch_path, &self.target.local_path).await {
            let refused = FetchError::Io {
                operation: LocalOperation::Renaming,
                path: self.target.local_path.clone(),
                cause,
            };
            discard_all(vec![self]).await;
            return Err(refused);
        }

        index
            .mark_present(LocalObservation {
                path: self.path().clone(),
                size: self.entry.size,
                mtime: self.entry.mtime,
                at: now,
            })
            .await?;

        debug!(
            container = %self.target.location.container_id,
            bytes = self.written,
            "placed a fetched Entry and marked it present",
        );
        Ok(self.path().clone())
    }

    /// Removes the temporary file, this placement having come to nothing.
    ///
    /// One that is already gone is the same outcome as one this call removed, so
    /// a cleanup that races the failure it is cleaning up after still succeeds.
    pub(super) async fn discard(self) -> FetchResult<()> {
        drop(self.file);
        match fs::remove_file(&self.scratch_path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(cause) => Err(FetchError::Io {
                operation: LocalOperation::Removing,
                path: self.scratch_path,
                cause,
            }),
        }
    }

    /// Where in the Library this placement stands.
    fn path(&self) -> &EntryPath {
        self.target.path()
    }

    /// Sets the file's modification time to the one the Entry carries
    /// (spec: FM-9).
    ///
    /// Set on the temporary file rather than after the rename, so the file that
    /// appears at the final path is already stamped: a scan that ran between the
    /// two would otherwise see a file whose time is neither the Entry's nor
    /// anything the device wrote down.
    ///
    /// Synchronous, because setting times is a metadata call on a handle and
    /// `tokio::fs` offers none; it is one syscall on a file this run just wrote.
    async fn stamp(&self) -> FetchResult<()> {
        let refused = |cause| FetchError::Io {
            operation: LocalOperation::Stamping,
            path: self.scratch_path.clone(),
            cause,
        };
        let modified = system_time_of(self.entry.mtime).ok_or_else(|| {
            refused(io::Error::new(
                io::ErrorKind::InvalidInput,
                "an Entry's modification time this platform's clock cannot reach",
            ))
        })?;

        let path: PathBuf = self.scratch_path.clone();
        tokio::task::spawn_blocking(move || {
            std::fs::File::options()
                .write(true)
                .open(&path)?
                .set_times(std::fs::FileTimes::new().set_modified(modified))
        })
        .await
        .map_err(|joined| refused(io::Error::other(joined)))?
        .map_err(refused)
    }
}

/// Publishes a whole Container's verified placements, discarding the rest if
/// one of them cannot be published.
///
/// The renames happen one after another and are not undone: each is a file that
/// is fully verified whichever of its neighbours fails, and a run that stopped
/// half way has placed those and reported the failure. What it does not do is
/// walk away from the temporary files it had not got to yet.
pub(super) async fn publish_all(
    index: &dyn Index,
    now: DeviceTime,
    placements: Vec<Placement<'_>>,
) -> FetchResult<Vec<EntryPath>> {
    let mut placed = Vec::with_capacity(placements.len());
    let mut left = placements.into_iter();
    while let Some(placement) = left.next() {
        match placement.publish(index, now).await {
            Ok(path) => placed.push(path),
            Err(error) => {
                discard_all(left.collect()).await;
                return Err(error);
            }
        }
    }
    Ok(placed)
}

/// Removes every temporary file a failed fetch left.
///
/// A cleanup failure is reported and not raised: what the caller is about to
/// report is the failure that made the cleanup necessary, and replacing it with
/// "and the temporary file would not go either" would lose the verdict. The path
/// stays out of the event, as it stays out of a message (spec: EP-1).
pub(super) async fn discard_all(placements: Vec<Placement<'_>>) {
    for placement in placements {
        if let Err(error) = placement.discard().await {
            warn!(
                operation = %LocalOperation::Removing,
                error = %error,
                "a fetch could not remove one of its own temporary files",
            );
        }
    }
}

/// The directory the Entry's file belongs in.
///
/// A translated local path always has a parent — it is a mapping's local root
/// with at least one component pushed onto it — so the absence of one is a
/// broken translation rather than a filesystem answer.
fn parent(local_path: &Path) -> FetchResult<&Path> {
    local_path.parent().ok_or_else(|| FetchError::Io {
        operation: LocalOperation::Creating,
        path: local_path.to_path_buf(),
        cause: io::Error::new(
            io::ErrorKind::InvalidInput,
            "a local path with no directory above it",
        ),
    })
}

/// What the operating system refused, with the file it refused it for.
fn refused(path: &Path, operation: LocalOperation) -> impl FnOnce(io::Error) -> FetchError + '_ {
    move |cause| FetchError::Io {
        operation,
        path: path.to_path_buf(),
        cause,
    }
}
