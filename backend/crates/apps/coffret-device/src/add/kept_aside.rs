use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};

use coffret_model::{EntryPath, Redacted};
use coffret_usecase::scratch::ScratchName;
use coffret_usecase::KeptFile;
use tracing::warn;

use crate::error::{Error, Result};

/// What stood where an [`IncomingFile`](super::IncomingFile) is about to be
/// kept, and what was kept of it
/// ([`keep_aside`](super::IncomingFile::keep_aside)).
#[derive(Debug)]
pub enum Replacing {
    /// Nothing the file replaces: its name is free, or a folder stands there,
    /// which the rename refuses to replace.
    Nothing,
    /// A file stood there, and it is kept aside until the value is dropped or
    /// put back.
    Kept(KeptAside),
    /// A file stood there, and this folder's filesystem would not keep it
    /// aside: what the add writes over it cannot be undone (spec: LA-36).
    /// The refusal says what the filesystem answered.
    Unkept(Error),
}

/// The file an add is about to write over, kept aside in the same folder under
/// a scratch name, so that stopping the add can put it back (spec: LA-36).
///
/// The name is held for as long as the value lives, so a sync in this process
/// leaves the kept file alone however long the add takes to commit, and the
/// gateway holds a lock on it for a sync in another (spec: OC-8). Dropping the
/// value lets the kept file go, which is what an add that committed or ended
/// does with what it kept; one that was put back leaves nothing to let go.
pub struct KeptAside {
    /// Where in the Library the file stands, which a refusal is spelled in.
    path: EntryPath,
    file: Box<dyn KeptFile>,
    /// Whether it went back to its name, so there is nothing to let go.
    put_back: AtomicBool,
    /// Held until the kept file is put back or let go, and dropped after the
    /// value's own [`Drop`] has done either.
    _name: ScratchName,
}

impl KeptAside {
    /// The file at `path`, kept by `file` under `name`.
    pub(super) fn new(path: EntryPath, file: Box<dyn KeptFile>, name: ScratchName) -> Self {
        Self {
            path,
            file,
            put_back: AtomicBool::new(false),
            _name: name,
        }
    }

    /// Puts the kept file back at its name, replacing what stands there: the
    /// caller has made sure that is still the file the add wrote.
    ///
    /// One rename within the folder, so the name carries either file and never
    /// neither (spec: EP-11).
    ///
    /// # Errors
    ///
    /// [`Error::Local`](crate::Error::Local) carrying
    /// [`Renaming`](coffret_usecase::LocalOperation::Renaming) where the rename
    /// was refused; the kept file is let go when the value is dropped then, and
    /// the add's file stays at the name.
    pub fn put_back(&self) -> Result<()> {
        self.file
            .put_back()
            .map_err(|refused| Error::below_root(refused, &self.path))?;
        self.put_back.store(true, Ordering::Release);
        Ok(())
    }
}

impl fmt::Debug for KeptAside {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("KeptAside")
            .field("put_back", &self.put_back.load(Ordering::Acquire))
            .finish_non_exhaustive()
    }
}

impl Drop for KeptAside {
    fn drop(&mut self) {
        if self.put_back.load(Ordering::Acquire) {
            return;
        }
        // Reported and not raised, as a scratch an upload left is: the
        // reserved prefix keeps a scan from reading it, and once the name is
        // let go a later sync removes it as stale (spec: OC-8). The path stays
        // out of the event (spec: EL-1).
        if let Err(refused) = self.file.remove() {
            warn!(
                operation = "add_file",
                error = %refused.redacted(),
                "a file kept aside for an add could not be let go; a later sync removes it",
            );
        }
    }
}
