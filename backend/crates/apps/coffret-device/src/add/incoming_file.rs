use std::path::PathBuf;

use coffret_model::{EntryPath, Redacted};
use coffret_usecase::scratch;
use coffret_usecase::{Destination, ScratchFile};
use tracing::{debug, warn};

use crate::error::{Error, Result};

/// One file on its way into a mapped folder.
///
/// **Where** it may go is the mappings' answer (spec: EP-9), and this crate does
/// not read it a second time: what `open` is handed is a [`Destination`], the
/// folder a descent from the mapped root left open having refused to pass
/// through anything that is not a real folder of that root — a path it would not
/// descend is refused rather than written somewhere else, on the
/// no-silent-selection posture EP-4 sets. Every call below — the scratch,
/// the rename, the removal — is then made against that open folder rather than
/// against a path, so nothing can change under the answer between the descent
/// and the write.
///
/// **When** it exists is EP-11's, and for EP-11's reason. The bytes go into a
/// scratch beside their destination as they arrive, the file is flushed,
/// and only then is it renamed onto the final name. A rename within one
/// directory is atomic, so what a scan or a reader can see at that path is
/// either nothing or the whole file — never a prefix of one, which is what a
/// sync would otherwise commit as an Entry the person never had.
///
/// The order is the capability's types rather than this type's discipline: a
/// [`ScratchFile`] is the only thing that can be written and what its flush
/// hands back is the only thing that can be published, so nothing here can
/// publish bytes that are not on the device.
///
/// Its scratch name comes from coffret's reserved prefix
/// ([`scratch`](coffret_usecase::scratch)), so a transfer that stops halfway
/// leaves a name the scan steps over rather than one it reads as user data.
///
/// A value that is dropped without [`keep`](Self::keep) removes its scratch:
/// an abandoned upload is a request that went away, and the folder should not
/// accumulate what it left. That is what makes dropping it the right thing to
/// do with one — there is no state to unwind and nothing to report, because
/// nothing has become visible.
///
/// Nothing here writes to the catalog, and nothing should. A file this device
/// did not materialize from an Entry has no local row to make (spec: EP-10):
/// what makes it part of the Library is the next sync, which finds it the same
/// way it finds a file copied in by hand.
pub struct IncomingFile {
    /// Where in the Library the file will stand, which is what a refusal about
    /// it is spelled in.
    path: EntryPath,
    /// The top-level component the mapping this folder was reached through
    /// stands for, or `None` for the Library root.
    ///
    /// Held because a refusal a descent reports names the mapping it is about
    /// (spec: EP-13), and this value is the only thing here that knows which
    /// mapping the open folder came from: what the writes go through is one
    /// folder, and a folder says nothing about the row that chose it.
    ///
    /// Not because any call below can raise that refusal. A root is held against
    /// the identity its mapping recorded by
    /// [`Destinations::reach`](coffret_usecase::Destinations::reach) and by
    /// nothing else, so the only descent that can be refused is the one
    /// [`receive_file`](crate::OpenLibrary::receive_file) made before this value
    /// existed — creating, writing, flushing and publishing against a folder
    /// that is already open cannot. What the field buys is that the four sites
    /// here go through the same `Error::descent` as that one, rather than a
    /// second, narrower reading of what a descent can report.
    prefix: Option<EntryPath>,
    /// The destination folder, held open from the descent until the rename.
    directory: Box<dyn Destination>,
    /// What the bytes are called until they are all there, and `None` once the
    /// rename has happened — which is what tells the drop guard there is nothing
    /// left to remove.
    scratch_name: Option<String>,
    /// The open scratch, until it is flushed.
    file: Option<Box<dyn ScratchFile>>,
    written: u64,
}

impl IncomingFile {
    /// Opens a scratch in the folder a descent arrived at.
    ///
    /// The folders above it were made by that descent, because a person dropping
    /// a folder is adding the folders in it: an Entry Path's separators are the
    /// whole of what a folder is (spec: EP-2), so there is nothing else for a
    /// subpath to mean. What the descent would not make is a folder reached
    /// through a symbolic link, which is why the caller does it before it gets
    /// here (spec: EP-4, EP-11).
    pub(super) async fn open(
        path: EntryPath,
        prefix: Option<EntryPath>,
        directory: Box<dyn Destination>,
    ) -> Result<Self> {
        let scratch_name = scratch::incoming_name();
        let file = directory
            .create(&scratch_name)
            .map_err(|refused| Error::descent(refused, prefix.as_ref(), &path))?;

        Ok(Self {
            path,
            prefix,
            directory,
            scratch_name: Some(scratch_name),
            file: Some(file),
            written: 0,
        })
    }

    /// Takes the next piece of the file.
    pub async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        let file = self
            .file
            .as_mut()
            .expect("an incoming file is written to before it is kept");
        // The refusal is turned into this crate's words once the write has let
        // go of the handle, because saying what it was about reads the value.
        if let Err(refused) = file.write(bytes).await {
            return Err(Error::descent(refused, self.prefix.as_ref(), &self.path));
        }
        self.written += bytes.len() as u64;
        Ok(())
    }

    /// Flushes what was written and renames it onto its final name, which is the
    /// moment the file exists.
    ///
    /// Flushed before the rename because the rename is what publishes the file: a
    /// crash that reordered the two would leave a name promising content this
    /// device never wrote. Both names are resolved against the folder the descent
    /// left open, so the file lands where that descent arrived rather than
    /// wherever the path would resolve to now.
    ///
    /// An existing file at the path is replaced, which is what a rename does and
    /// what this means to do: the caller has already decided that writing here is
    /// allowed, and a replacement is a change the next sync carries into the
    /// Library like any other — a local file differing from its current Entry is
    /// what makes it eligible, and the Container holding that Entry is what gets
    /// replaced (spec: PK-11, PK-12).
    pub async fn keep(mut self) -> Result<()> {
        let file = self
            .file
            .take()
            .expect("an incoming file is flushed before it is kept");
        let flushed = match file.flush().await {
            Ok(flushed) => flushed,
            // The scratch is what the failure leaves behind, and the drop
            // guard still holds the name it is called by, so it is taken by that
            // guard as this value goes out of scope.
            Err(refused) => return Err(Error::descent(refused, self.prefix.as_ref(), &self.path)),
        };

        let scratch_name = self
            .scratch_name
            .take()
            .expect("a file is kept exactly once");
        if let Err(refused) = flushed.publish() {
            // The scratch is still there, and the drop guard is what
            // would have taken it — so its name is put back on the value before
            // the error goes out.
            self.scratch_name = Some(scratch_name);
            return Err(Error::descent(refused, self.prefix.as_ref(), &self.path));
        }

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

    /// Where the scratch being written stands, for an error to name — or
    /// for a caller with something to ask the filesystem about the volume these
    /// bytes are landing on.
    ///
    /// Present from the moment
    /// [`receive_file`](crate::OpenLibrary::receive_file) hands this value back
    /// until [`keep`](Self::keep) takes the name, and every method that reaches
    /// for it runs in between — a value that had been kept has been consumed, so
    /// no caller is left holding one to write to. Nothing reaches the filesystem
    /// through it: the writes go through the open folder, and a caller that takes
    /// this path may ask about it but must not write through it.
    pub fn scratch_path(&self) -> PathBuf {
        let name = self
            .scratch_name
            .as_deref()
            .expect("an incoming file is written to before it is kept");
        self.directory.path_of(name)
    }
}

impl Drop for IncomingFile {
    fn drop(&mut self) {
        let Some(scratch_name) = self.scratch_name.take() else {
            return;
        };
        // Closed before the name it holds is removed.
        drop(self.file.take());
        // Synchronous, and deliberately: a drop cannot await, and spawning a task
        // to remove one file would outlive the runtime a request was served on.
        // The capability's removal is synchronous for exactly this reason — it is
        // one call against a folder this value has held open all along, and one
        // that is already gone is the outcome this wanted anyway.
        match self.directory.remove(&scratch_name) {
            Ok(()) => debug!(
                operation = "add_file",
                bytes = self.written,
                "an upload that did not finish left nothing behind",
            ),
            // Reported and not raised: the scratch prefix already keeps a scan
            // from reading it as user data, so what is lost is tidiness rather
            // than correctness. The refusal's log-safe rendering is what keeps
            // the path out of the event (spec: EL-1).
            Err(cause) => warn!(
                operation = "add_file",
                error = %cause.redacted(),
                "an upload that did not finish left a scratch behind",
            ),
        }
    }
}
