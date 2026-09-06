use std::path::{Path, PathBuf};

use crate::spool::Spool;

/// What a backend hands the spool suite for one case.
///
/// One [`Spool`] and one directory to use it in, and the directory is handed
/// over *unprepared*: making it is [`Spool::prepare_dir`]'s to do, and a case
/// about a file created under a directory nobody made needs somewhere that was
/// never made.
pub struct SpoolUnderTest {
    // Dropped before `resources`, so that whatever the spool is kept in
    // outlives it.
    spool: Box<dyn Spool>,
    dir: PathBuf,
    resources: Vec<Box<dyn Send + Sync>>,
}

impl SpoolUnderTest {
    /// Takes a spool and a directory the case may write under.
    pub fn new(spool: Box<dyn Spool>, dir: impl AsRef<Path>) -> Self {
        Self {
            spool,
            dir: dir.as_ref().to_path_buf(),
            resources: Vec::new(),
        }
    }

    /// Keeps something alive for as long as the case runs.
    ///
    /// A backend whose directory is a temporary one hands the owner over here
    /// rather than leaking it.
    pub fn holding(mut self, resource: Box<dyn Send + Sync>) -> Self {
        self.resources.push(resource);
        self
    }

    /// The spool under test.
    pub fn spool(&self) -> &dyn Spool {
        self.spool.as_ref()
    }

    /// The directory the case spools into.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}
