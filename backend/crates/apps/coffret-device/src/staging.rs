//! Building a Library directory somewhere it cannot be mistaken for a finished
//! one.
//!
//! Both ways a Library appears on a device — created here, or joined from a
//! Recovery Code — write the same five things in the same order, and both have
//! to be abandonable at any point. So both build in a directory named after
//! neither: the directory takes the Library's real name in one rename once the
//! last step has landed, which is what makes a directory under the real name
//! always a whole Library and an interrupted attempt something a later one can
//! discard rather than half a Library nothing can tell from a whole one.

use std::fs;
use std::path::PathBuf;

use tracing::{info, warn};

use crate::error::{CreationStep, Error, Result};
use crate::library_dir::LibraryDir;
use crate::owner_only;

/// Which way a Library is coming to be on this device.
///
/// The one word the two flows differ in: a failure has to say whether the
/// Library was not created or not joined, because only one of the two can have
/// left anything on Storage behind.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Flow {
    /// A Library that did not exist anywhere until now.
    Creating,
    /// A Library another device created, being taken up by this one.
    Joining,
}

impl Flow {
    /// What a run of this flow is recorded as.
    fn operation(self) -> &'static str {
        match self {
            Self::Creating => "create_library",
            Self::Joining => "join_library",
        }
    }
}

/// One attempt at putting a Library on this device.
pub(crate) struct Staging {
    flow: Flow,
    /// Where the finished Library goes.
    dir: LibraryDir,
    /// Where it is built until then.
    staging: LibraryDir,
    /// The app folder this attempt cannot take back, where it created one.
    folder: Option<String>,
}

impl Staging {
    /// Where a Library called `name` would go, if nothing is there already.
    ///
    /// The two refusals a name alone can earn — it is not one path component, or
    /// a Library of that name is already on this device — are made here, before
    /// a directory exists, before Storage is asked anything, and before anybody
    /// is asked for a Passphrase. Both are answers a person acts on and neither
    /// costs a file.
    pub(crate) fn vacant(name: &str) -> Result<LibraryDir> {
        let dir = LibraryDir::resolve(name)?;
        if dir.is_present() {
            return Err(Error::LibraryExists {
                name: dir.name().to_owned(),
                path: dir.path().to_path_buf(),
            });
        }
        Ok(dir)
    }

    /// Opens the staging directory a Library is built in.
    pub(crate) fn begin(flow: Flow, dir: LibraryDir) -> Result<Self> {
        let staging = dir.staging();
        if staging.path().exists() {
            // Discarded rather than resumed: nothing in it reached Storage under
            // a key anything kept, so there is no state in it worth more than
            // the certainty of starting from nothing.
            fs::remove_dir_all(staging.path()).map_err(Error::local(
                "discarding what an interrupted attempt left",
                staging.path(),
            ))?;
            info!(
                operation = flow.operation(),
                "discarded a directory an interrupted attempt left"
            );
        }
        owner_only::create_dir("creating the Library directory", staging.path())?;

        Ok(Self {
            flow,
            dir,
            staging,
            folder: None,
        })
    }

    /// The directory every step of this attempt writes into.
    ///
    /// Never the one the Library will be known by: that name is taken in one
    /// rename, by [`publish`](Self::publish), once everything is written.
    pub(crate) fn staged(&self) -> &LibraryDir {
        &self.staging
    }

    /// Records the app folder this attempt created, which a failure from here
    /// on cannot take back.
    pub(crate) fn created_folder(&mut self, folder_id: String) {
        self.folder = Some(folder_id);
    }

    /// Reports the step that failed, and — where the flow is one that creates a
    /// folder — the folder this attempt cannot take back.
    pub(crate) fn failed(&self, step: CreationStep, cause: Error) -> Error {
        let name = self.dir.name().to_owned();
        let cause = Box::new(cause);
        match self.flow {
            Flow::Creating => Error::LibraryNotCreated {
                name,
                step,
                orphan_folder: self.folder.clone(),
                cause,
            },
            // No orphan folder: a join creates nothing on Storage, so there is
            // nothing it can leave there.
            Flow::Joining => Error::LibraryNotJoined { name, step, cause },
        }
    }

    /// Moves the finished directory to the name the Library is known by, and
    /// reports where it now is.
    pub(crate) fn publish(self) -> Result<PathBuf> {
        if let Err(cause) = fs::rename(self.staging.path(), self.dir.path()) {
            let failure = self.failed(
                CreationStep::Publish,
                Error::Local {
                    doing: "moving the finished Library directory into place",
                    path: self.dir.path().to_path_buf(),
                    cause,
                },
            );
            self.discard();
            return Err(failure);
        }
        Ok(self.dir.path().to_path_buf())
    }

    /// Removes what an attempt that did not finish had built so far.
    pub(crate) fn discard(self) {
        if let Err(cause) = fs::remove_dir_all(self.staging.path()) {
            // There is nothing to do about it and nothing that depends on it —
            // the Library is not on this device either way — so it is recorded
            // rather than reported over the failure that actually stopped the
            // attempt.
            warn!(
                operation = self.flow.operation(),
                reason = %cause,
                "could not remove what an interrupted attempt left"
            );
        }
    }
}
