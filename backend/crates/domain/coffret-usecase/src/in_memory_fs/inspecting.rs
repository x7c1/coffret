use std::path::{Path, PathBuf};

use coffret_model::{Btime, Mtime};

use crate::in_memory_fs::InMemoryFs;

impl InMemoryFs {
    /// The files directly under `dir`, in path order.
    ///
    /// What a case counting spools reads: a run that committed its batch leaves
    /// none, and one that was interrupted leaves exactly the ones its pending
    /// rows name (spec: OC-2).
    pub fn files_under(&self, dir: &Path) -> Vec<PathBuf> {
        self.state().files_under(dir)
    }

    /// One file's whole content, or `None` where nothing is at that path.
    pub fn content(&self, path: &Path) -> Option<Vec<u8>> {
        self.state().content(path)
    }

    /// One file's length and modification time, or `None` where nothing is at
    /// that path.
    ///
    /// What a case holds against what a run wrote down about a file
    /// (spec: EP-10).
    pub fn observed(&self, path: &Path) -> Option<(u64, Mtime)> {
        self.state().observed(path)
    }

    /// What the fake says a file was created at, if anything (spec: FM-9).
    pub fn born(&self, path: &Path) -> Option<Btime> {
        self.state().born(path)
    }
}
