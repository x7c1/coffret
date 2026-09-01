use std::io;
use std::path::{Path, PathBuf};

use coffret_usecase::scratch;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, warn};

use crate::error::{Error, Result};

/// One file on its way into a mapped folder.
///
/// The order is EP-11's, and for EP-11's reason. The bytes go into a temporary
/// file beside their destination as they arrive, the file is flushed, and only
/// then is it renamed onto the final path. A rename within one directory is
/// atomic, so what a scan or a reader can see at that path is either nothing or
/// the whole file — never a prefix of one, which is what a sync would otherwise
/// commit as an Entry the person never had.
///
/// The temporary name carries coffret's reserved scratch prefix
/// ([`scratch`](coffret_usecase::scratch)), so a transfer that stops halfway
/// leaves a name the scan steps over rather than one it reads as user data.
///
/// A value that is dropped without [`keep`](Self::keep) removes its temporary
/// file: an abandoned upload is a request that went away, and the folder should
/// not accumulate what it left. That is what makes dropping it the right thing
/// to do with one — there is no state to unwind and nothing to report, because
/// nothing has become visible.
///
/// Nothing here writes to the catalog, and nothing should. A file this device
/// did not materialize from an Entry has no local row to make (spec: EP-10):
/// what makes it part of the Library is the next sync, which finds it the same
/// way it finds a file copied in by hand.
pub struct IncomingFile {
    /// Where the bytes go until they are all there, and `None` once the rename
    /// has happened — which is what tells the drop guard there is nothing left
    /// to remove.
    scratch_path: Option<PathBuf>,
    /// Where they belong.
    destination: PathBuf,
    /// The open temporary file, until it is flushed.
    file: Option<fs::File>,
    written: u64,
}

impl IncomingFile {
    /// Opens a temporary file beside where the file will go, making the folders
    /// above it.
    ///
    /// The folders are created because a person dropping a folder is adding the
    /// folders in it: an Entry Path's separators are the whole of what a folder
    /// is (spec: EP-2), so there is nothing else for a subpath to mean.
    pub(super) async fn open(destination: PathBuf) -> Result<Self> {
        let directory = parent(&destination);
        fs::create_dir_all(directory)
            .await
            .map_err(Error::local("a folder could not be created", directory))?;

        let scratch_path = directory.join(scratch::incoming_name());
        let file = fs::File::create(&scratch_path)
            .await
            .map_err(Error::local("a file could not be created", &scratch_path))?;

        Ok(Self {
            scratch_path: Some(scratch_path),
            destination,
            file: Some(file),
            written: 0,
        })
    }

    /// Takes the next piece of the file.
    pub async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        let (path, file) = self.writing();
        file.write_all(bytes)
            .await
            .map_err(Error::local("a file could not be written", path))?;
        self.written += bytes.len() as u64;
        Ok(())
    }

    /// Flushes what was written and renames it onto its final path, which is the
    /// moment the file exists.
    ///
    /// Flushed before the rename because the rename is what publishes the file: a
    /// crash that reordered the two would leave a name promising content this
    /// device never wrote.
    ///
    /// An existing file at the path is replaced, which is what a rename does and
    /// what this means to do: the caller has already decided that writing here is
    /// allowed, and a replacement is a change the next sync carries into the
    /// Library like any other — a local file differing from its current Entry is
    /// what makes it eligible, and the Container holding that Entry is what gets
    /// replaced (spec: PK-11, PK-12).
    pub async fn keep(mut self) -> Result<()> {
        let (path, file) = self.writing();
        file.sync_all()
            .await
            .map_err(Error::local("a file could not be flushed", path))?;
        // Closed before the rename, so nothing is holding the name this call is
        // about to move.
        drop(self.file.take());

        let scratch_path = self
            .scratch_path
            .take()
            .expect("a file is kept exactly once");
        fs::rename(&scratch_path, &self.destination)
            .await
            .map_err(|cause| {
                // The temporary file is what the failure leaves behind, and the
                // drop guard is what would have taken it — so it is put back on
                // the value before the error goes out.
                self.scratch_path = Some(scratch_path);
                Error::local("a file could not be renamed into place", &self.destination)(cause)
            })?;

        debug!(
            operation = "add_file",
            bytes = self.written,
            "took a file into a mapped folder",
        );
        Ok(())
    }

    /// How many bytes have been written so far.
    pub fn written(&self) -> u64 {
        self.written
    }

    /// The temporary file being written, and the name it is under.
    ///
    /// Both are present from [`open`](Self::open) until [`keep`](Self::keep)
    /// takes them, and every method that reaches for them runs in between — a
    /// value that had been kept has been consumed, so no caller is left holding
    /// one to write to.
    fn writing(&mut self) -> (&Path, &mut fs::File) {
        let path = self
            .scratch_path
            .as_deref()
            .expect("an incoming file is written to before it is kept");
        let file = self
            .file
            .as_mut()
            .expect("an incoming file is written to before it is kept");
        (path, file)
    }
}

impl Drop for IncomingFile {
    fn drop(&mut self) {
        let Some(scratch_path) = self.scratch_path.take() else {
            return;
        };
        // Synchronous, and deliberately: a drop cannot await, and spawning a task
        // to remove one file would outlive the runtime a request was served on.
        // One that is already gone is the outcome this wanted anyway.
        match std::fs::remove_file(&scratch_path) {
            Ok(()) => debug!(
                operation = "add_file",
                bytes = self.written,
                "an upload that did not finish left nothing behind",
            ),
            Err(gone) if gone.kind() == io::ErrorKind::NotFound => {}
            // Reported and not raised: the scratch prefix already keeps a scan
            // from reading it as user data, so what is lost is tidiness rather
            // than correctness. The path stays out of the event (spec: EP-1).
            Err(cause) => warn!(
                operation = "add_file",
                error = %cause,
                "an upload that did not finish left a temporary file behind",
            ),
        }
    }
}

/// The directory the file belongs in.
///
/// A translated local path always has a parent — it is a mapping's local root
/// with at least one component pushed onto it (spec: EP-9) — so the absence of
/// one is a broken translation rather than a filesystem answer.
///
/// Which is why it is asserted rather than reported. The one failure this crate
/// has for a local path is [`Error::Local`](crate::Error::Local), whose cause is
/// what the operating system said about it; nothing was asked of the operating
/// system here, and an `io::Error` written to fill that field would put words in
/// its mouth for whoever reads the chain in the log.
fn parent(destination: &Path) -> &Path {
    destination
        .parent()
        .expect("a translated local path stands inside a mapped folder")
}
