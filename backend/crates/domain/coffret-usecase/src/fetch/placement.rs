use std::io;
use std::path::{Path, PathBuf};

use coffret_model::{ContentHash, EntryMetadata, EntryPath, Redacted};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, warn};

use crate::device_state::{DeviceTime, LocalObservation};
use crate::fetch::confined_dir::ConfinedDir;
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::target::Target;
use crate::index::Index;
use crate::local_operation::LocalOperation;
use crate::local_times::system_time_of;
use crate::scratch;

/// One Entry on its way into a mapped folder.
///
/// Two questions are settled here. **Where** the bytes may go is the mappings'
/// answer (spec: EP-9): the folder is reached by descending the Entry Path's
/// components from the mapped root one at a time, refusing anything that is not
/// a real folder of that root ([`ConfinedDir`]), and every call afterwards is
/// made against the folder the descent left open rather than against a path
/// joined back together. So the answer is taken once, with the folder held open,
/// and cannot go stale before the write — and a path that cannot be reached that
/// way is refused rather than placed somewhere else, on the no-silent-selection
/// posture EP-4 sets.
///
/// **When** the file becomes visible is EP-11's, and the order is the only one
/// that gives it. The bytes go into a temporary file *in that folder* as they
/// arrive, the file is flushed, it is checked against the hash the current
/// catalog records for the Entry, it is stamped with the Entry's own
/// modification time — and only then is it renamed onto the final name. A rename
/// within one directory is atomic, so what a reader can see at the target path is
/// either nothing or the whole verified file: never a prefix of one, never one
/// whose stamp has yet to be set, and never one whose content has not been held
/// against the catalog.
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
    /// The Entry, and where in the Library it stands.
    target: &'a Target,
    /// What the Container's own entry table records about it (spec: FM-9).
    entry: EntryMetadata,
    /// The destination folder, held open from the descent until the rename.
    directory: ConfinedDir,
    /// What the temporary file is called inside it.
    scratch_name: String,
    /// Where that file stands, for an error to name. Nothing reaches the
    /// filesystem through it — every call goes through `directory`.
    scratch_path: PathBuf,
    file: Option<fs::File>,
    /// The Entry's plaintext as it passes, for the check before the rename.
    hasher: blake3::Hasher,
    written: u64,
}

impl<'a> Placement<'a> {
    /// Descends to the folder the Entry's file belongs in and opens a temporary
    /// file inside it.
    ///
    /// The descent makes the folders that are not there yet, because an Entry
    /// Path's separators are the whole of what a folder is (spec: EP-2), and
    /// refuses to pass through anything that is not a folder of the mapped root
    /// (spec: EP-4, EP-11). A path it will not descend is reported as
    /// [`FetchError::UnmaterializablePath`], which is the verdict every other
    /// path this device cannot make a file for already gets.
    ///
    /// It fails the run rather than becoming a finding, and that is the
    /// difference between here and the selection. The selection descended to
    /// this same place and found it sound; a fence met now is a name that has
    /// become a symbolic link since, which is a race on the disk rather than the
    /// shape it was in when the run was planned.
    ///
    /// `entry` is the Container's own account of the Entry rather than the
    /// catalog's: it says how many bytes of the plaintext stream belong to this
    /// Entry, and holding the two accounts against each other is
    /// [`verify`](Self::verify)'s.
    pub(super) async fn open(target: &'a Target, entry: EntryMetadata) -> FetchResult<Self> {
        let directory = target
            .place
            .descend()
            .await
            .map_err(|refused| FetchError::from_descent(refused, target.path()))?;

        let scratch_name = scratch::name(target.location.container_id);
        let file = directory
            .create(&scratch_name)
            .map_err(|refused| FetchError::from_descent(refused, target.path()))?;
        let scratch_path = directory.path_of(&scratch_name);

        Ok(Self {
            target,
            entry,
            directory,
            scratch_name,
            scratch_path,
            file: Some(fs::File::from_std(file)),
            hasher: blake3::Hasher::new(),
            written: 0,
        })
    }

    /// Where this Entry's plaintext starts in its Container's stream
    /// (spec: FM-9).
    pub(super) fn start(&self) -> u64 {
        self.entry.extent.offset()
    }

    /// Where it ends.
    pub(super) fn end(&self) -> u64 {
        self.entry.extent.end()
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

        let hash = ContentHash::from_bytes(*self.hasher.finalize().as_bytes());
        if self.written != self.entry.extent.size() || hash != self.target.location.entry.hash {
            return Err(FetchError::ContentMismatch {
                container_id: self.target.location.container_id,
                path: self.path().clone(),
            });
        }
        self.stamp(file).await
    }

    /// Renames the verified file onto its final name and records having placed
    /// it.
    ///
    /// Both names are resolved against the folder the descent left open, so the
    /// file lands where the descent arrived whatever the path above it has
    /// become since (spec: EP-4, EP-11).
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
        if let Err(cause) = self.directory.publish(&self.scratch_name) {
            let refused = FetchError::from_descent(cause, self.target.path());
            discard_all(vec![self]).await;
            return Err(refused);
        }

        index
            .mark_present(LocalObservation {
                path: self.path().clone(),
                size: self.entry.extent.size(),
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
        self.directory
            .remove(&self.scratch_name)
            .map_err(|refused| FetchError::from_descent(refused, self.target.path()))
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
    /// Set on the handle this run has been writing to rather than by opening the
    /// name again, which is both one syscall fewer and the only form that keeps
    /// the confinement: reopening by path would be the one place a symbolic link
    /// could get between the descent and the stamp. It happens off the runtime's
    /// threads because setting times is a metadata call on a handle and
    /// `tokio::fs` offers none.
    async fn stamp(&self, file: fs::File) -> FetchResult<()> {
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

        let file = file.into_std().await;
        tokio::task::spawn_blocking(move || {
            file.set_times(std::fs::FileTimes::new().set_modified(modified))
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
                error = %error.redacted(),
                "a fetch could not remove one of its own temporary files",
            );
        }
    }
}

/// What the operating system refused, with the file it refused it for.
fn refused(path: &Path, operation: LocalOperation) -> impl FnOnce(io::Error) -> FetchError + '_ {
    move |cause| FetchError::Io {
        operation,
        path: path.to_path_buf(),
        cause,
    }
}
