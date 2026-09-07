use std::path::{Path, PathBuf};

use crate::mapped_roots::MappedRoots;
use crate::mapped_roots_conformance::folder_arrangement::FolderArrangement;

/// What a backend hands the mapped-roots suite for one case.
///
/// The capability under test, a way to arrange the folder it reads, and one
/// directory to work inside — handed over *made*, unlike the spool suite's,
/// because what a mapped root is starts with it being there. The cases that want
/// a missing root name a path under it that nobody created.
pub struct MappedRootsUnderTest {
    // Dropped before `resources`, so that whatever the capability is kept in
    // outlives it.
    roots: Box<dyn MappedRoots>,
    arrange: Box<dyn FolderArrangement>,
    dir: PathBuf,
    resources: Vec<Box<dyn Send + Sync>>,
}

impl MappedRootsUnderTest {
    /// Takes a capability, the arrangement that goes behind it, and a directory
    /// that already exists.
    pub fn new(
        roots: Box<dyn MappedRoots>,
        arrange: Box<dyn FolderArrangement>,
        dir: impl AsRef<Path>,
    ) -> Self {
        Self {
            roots,
            arrange,
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

    /// The capability under test.
    pub fn roots(&self) -> &dyn MappedRoots {
        self.roots.as_ref()
    }

    /// How the case puts folders and files where the capability will find them.
    pub fn arrange(&self) -> &dyn FolderArrangement {
        self.arrange.as_ref()
    }

    /// The directory the case works inside.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}
