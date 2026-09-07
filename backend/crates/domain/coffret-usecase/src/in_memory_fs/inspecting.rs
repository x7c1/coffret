use std::path::{Path, PathBuf};

use coffret_model::{Btime, Mtime};

use crate::in_memory_fs::state::lock;
use crate::in_memory_fs::InMemoryFs;

impl InMemoryFs {
    /// The files directly under `dir`, in path order.
    ///
    /// What a case counting spools reads: a run that committed its batch leaves
    /// none, and one that was interrupted leaves exactly the ones its pending
    /// rows name (spec: OC-2).
    pub fn files_under(&self, dir: &Path) -> Vec<PathBuf> {
        lock(&self.state).files_under(dir)
    }

    /// One file's whole content, or `None` where nothing is at that path.
    pub fn content(&self, path: &Path) -> Option<Vec<u8>> {
        lock(&self.state).content(path)
    }

    /// One file's length and modification time, or `None` where nothing is at
    /// that path.
    ///
    /// What a case holds against what a run wrote down about a file
    /// (spec: EP-10).
    pub fn observed(&self, path: &Path) -> Option<(u64, Mtime)> {
        lock(&self.state).observed(path)
    }

    /// What the fake says a file was created at, if anything (spec: FM-9).
    pub fn born(&self, path: &Path) -> Option<Btime> {
        lock(&self.state).born(path)
    }

    /// Every file anywhere beneath `dir`, in path order.
    ///
    /// What a case counting a fetch's leftovers reads: a temporary file lands
    /// beside its Entry's own destination, so the question is about the whole
    /// subtree rather than about one folder (spec: EP-11).
    pub fn files_beneath(&self, dir: &Path) -> Vec<PathBuf> {
        lock(&self.state).files_beneath(dir)
    }

    /// Whether anything at all stands at `path` — a file, a folder, or a planted
    /// "other".
    ///
    /// What a case asking "is the place still empty" reads, and it deliberately
    /// does not say which of the three: a name a fetch may not write at is one
    /// name whichever of them is standing there (spec: EP-11).
    pub fn holds(&self, path: &Path) -> bool {
        lock(&self.state).holds(path)
    }
}
