use std::path::{Path, PathBuf};

use crate::destinations::Destinations;
use crate::destinations_conformance::root_arrangement::RootArrangement;

/// What a backend hands the destinations suite for one case.
///
/// The capability under test, a way to arrange the root it writes into and read
/// it back, and one mapped root to work inside — handed over *made*, because a
/// mapped root is the folder a person pointed this device at and every case
/// starts from one that is there. The folders *below* it are the capability's own
/// to make, which is what one of the cases is about (spec: EP-2).
pub struct DestinationsUnderTest {
    // Dropped before `resources`, so that whatever the capability is kept in
    // outlives it.
    destinations: Box<dyn Destinations>,
    arrange: Box<dyn RootArrangement>,
    root: PathBuf,
    resources: Vec<Box<dyn Send + Sync>>,
}

impl DestinationsUnderTest {
    /// Takes a capability, the arrangement that goes behind it, and a mapped
    /// root that already exists.
    pub fn new(
        destinations: Box<dyn Destinations>,
        arrange: Box<dyn RootArrangement>,
        root: impl AsRef<Path>,
    ) -> Self {
        Self {
            destinations,
            arrange,
            root: root.as_ref().to_path_buf(),
            resources: Vec::new(),
        }
    }

    /// Keeps something alive for as long as the case runs.
    ///
    /// A backend whose root is a temporary directory hands the owner over here
    /// rather than leaking it.
    pub fn holding(mut self, resource: Box<dyn Send + Sync>) -> Self {
        self.resources.push(resource);
        self
    }

    /// The capability under test.
    pub fn destinations(&self) -> &dyn Destinations {
        self.destinations.as_ref()
    }

    /// How the case plants folders and files, and reads back what a placement
    /// left.
    pub fn arrange(&self) -> &dyn RootArrangement {
        self.arrange.as_ref()
    }

    /// The mapped root the case places under.
    pub fn root(&self) -> &Path {
        &self.root
    }
}
