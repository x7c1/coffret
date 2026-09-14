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
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use coffret_model::Redacted;
use coffret_usecase::{LocalIoError, LocalOperation};
use tracing::{debug, info, warn};

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
        // What a previous attempt left is discarded rather than resumed:
        // nothing in it reached Storage under a key anything kept, so there is
        // no state in it worth more than the certainty of starting from
        // nothing. Taking it out is one of the removals this device makes for
        // its own purposes, which are idempotent by rule, so an attempt
        // interrupted at any point costs the next one nothing beyond this
        // removal (spec: OC-8).
        match discard_directory(staging.path())? {
            Discarded::Directory => info!(
                operation = flow.operation(),
                "discarded a directory an interrupted attempt left"
            ),
            // Which is what every attempt but one following an interrupted one
            // finds, and what one whose directory another run took out first
            // finds as well: the removal ran, and what it was for is done.
            Discarded::Nothing => debug!(
                operation = flow.operation(),
                "no directory an interrupted attempt left was there to discard"
            ),
        }
        owner_only::create_dir(staging.path())?;

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
                LocalIoError::new(LocalOperation::Renaming, self.dir.path(), cause).into(),
            );
            self.discard();
            return Err(failure);
        }
        Ok(self.dir.path().to_path_buf())
    }

    /// Removes what an attempt that did not finish had built so far.
    ///
    /// Nothing is checked first: this is the device clearing away what it wrote
    /// for its own purposes, where absence is the outcome being sought rather
    /// than a state to find out about (spec: OC-8). A directory already gone is
    /// that outcome and is recorded as nothing at all — what is recorded below
    /// is a refusal, which is the only thing here a person could ever act on.
    pub(crate) fn discard(self) {
        if let Err(refused) = discard_directory(self.staging.path()) {
            // There is nothing to do about it and nothing that depends on it —
            // the Library is not on this device either way — so it is recorded
            // rather than reported over the failure that actually stopped the
            // attempt.
            record_cleanup_failure(self.flow, &refused);
        }
    }
}

/// What a removal of a staging directory took out.
#[derive(Debug)]
enum Discarded {
    /// A directory an interrupted attempt left, now gone.
    Directory,
    /// Nothing, because nothing was there.
    Nothing,
}

/// Takes out a staging directory, and says which of the two it took out.
///
/// Nothing is asked about the directory first, and one that is already gone is
/// the removal having happened rather than a failure: this is the device
/// clearing away what it wrote for its own purposes, where absence is the
/// outcome being sought (spec: OC-8). Swallowing it here is what keeps both
/// callers from reading an `ErrorKind` to find out which of the two they got.
///
/// Which of the two is handed back rather than recorded here, because only one
/// caller has anything to say about it: `begin` is discarding what a previous
/// attempt left and reports which of the two it met, while `discard` is taking
/// out what its own attempt built and says nothing either way. A message
/// recorded here would be `begin`'s, told about the wrong attempt.
fn discard_directory(path: &Path) -> std::result::Result<Discarded, LocalIoError> {
    match fs::remove_dir_all(path) {
        Ok(()) => Ok(Discarded::Directory),
        Err(gone) if gone.kind() == ErrorKind::NotFound => Ok(Discarded::Nothing),
        Err(cause) => Err(LocalIoError::new(LocalOperation::Removing, path, cause)),
    }
}

/// Records a cleanup refusal by its operation and error kind, never by the
/// staging directory or an operating-system message that may repeat it.
fn record_cleanup_failure(flow: Flow, refused: &LocalIoError) {
    warn!(
        operation = flow.operation(),
        reason = %refused.redacted(),
        "could not remove what an interrupted attempt left"
    );
}

#[cfg(test)]
mod tests {
    use std::io;

    use coffret_logging::testing::CapturedLogs;
    use tracing::Level;

    use super::*;
    use crate::testing::state_dir;

    // EL-1: an I/O message can repeat a private local path, including the name
    // this device gave the Library. The event keeps the useful operation and
    // error kind and neither copy of that location.
    #[test]
    fn cleanup_records_the_error_kind_without_the_private_location() {
        const LIBRARY: &str = "Family Tax Records";
        const PRIVATE_PATH: &str =
            "/Users/alice/Library/Application Support/coffret/libraries/Family Tax Records.partial";
        let logs = CapturedLogs::capture();
        let refused = LocalIoError::new(
            LocalOperation::Removing,
            PRIVATE_PATH,
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("could not remove {PRIVATE_PATH}"),
            ),
        );

        record_cleanup_failure(Flow::Creating, &refused);

        let event = logs.only(Level::WARN);
        assert_eq!(event.field("operation"), "create_library");
        assert_eq!(
            event.field("reason"),
            "Local::Io(operation=removed, kind=PermissionDenied)"
        );
        logs.assert_free_of(&[PRIVATE_PATH, LIBRARY, "Application Support"]);
    }

    // A directory that is already gone is a removal that worked (spec: OC-8),
    // and a removal that worked is not a cleanup failure. Nothing downstream
    // reads this — a discard hands nothing back — but a log saying a cleanup
    // was refused where none was is a log nobody can read correctly.
    #[test]
    fn a_discard_of_a_staging_directory_that_is_already_gone_records_nothing() {
        let attempt = Staging::begin(Flow::Creating, library("discard-of-what-is-already-gone"))
            .expect("an attempt must open its staging directory");
        fs::remove_dir_all(attempt.staged().path())
            .expect("what stands in for an interrupted clean-up must take the directory out");

        let logs = CapturedLogs::capture();
        attempt.discard();

        assert!(
            logs.events().is_empty(),
            "a removal that found what it wanted has nothing to report:\n{}",
            logs.text()
        );
    }

    // The state a race leaves: a directory an interrupted attempt left, taken
    // out by another run — or by a person clearing space — before this
    // attempt's removal reaches it. What the removal meets is absence, which
    // OC-8 calls a successful removal, so the attempt goes on. The DEBUG event
    // is what says the removal ran at all rather than being skipped by a look
    // that decided for it.
    #[test]
    fn a_removal_under_begin_still_runs_where_the_directory_is_already_gone() {
        let dir = library("staging-removed-under-begin");
        let staging = dir.staging();
        fs::create_dir_all(staging.path())
            .expect("what an interrupted attempt left must be there to be removed");
        fs::remove_dir_all(staging.path())
            .expect("what stands in for the other run must take the directory out");

        let logs = CapturedLogs::capture();
        let attempt = Staging::begin(Flow::Creating, dir)
            .expect("a removal that met a directory already gone is a removal that worked");

        assert!(
            attempt.staged().path().is_dir(),
            "the attempt must have its own staging directory to build in"
        );
        let event = logs.only(Level::DEBUG);
        assert_eq!(event.field("operation"), "create_library");
        // The other branch is the one that would say something untrue here —
        // that a directory an interrupted attempt left was discarded, when the
        // removal met nothing at all — so it is asserted against rather than
        // left to the DEBUG event to imply.
        assert!(
            logs.at(Level::INFO).is_empty(),
            "nothing was taken out, so nothing may say a directory was:\n{}",
            logs.text()
        );
        assert!(
            logs.at(Level::WARN).is_empty() && logs.at(Level::ERROR).is_empty(),
            "nothing failed here:\n{}",
            logs.text()
        );
    }

    // Absence is the one refusal the rule turns into an outcome. Anything else
    // is still a cleanup that could not be done, and is still recorded as one.
    #[test]
    fn a_discard_refused_for_anything_but_absence_is_still_recorded() {
        let attempt = Staging::begin(
            Flow::Creating,
            library("discard-refused-for-another-reason"),
        )
        .expect("an attempt must open its staging directory");
        put_a_file_where_the_directory_would_be(attempt.staged().path());

        let logs = CapturedLogs::capture();
        attempt.discard();

        let event = logs.only(Level::WARN);
        assert_eq!(event.field("operation"), "create_library");
        assert_eq!(
            event.field("reason"),
            "Local::Io(operation=removed, kind=NotADirectory)"
        );
    }

    // The same line for `begin`, where a refusal that is not absence still
    // stops the attempt: the directory it was about to build in is occupied by
    // something it could not take out.
    #[test]
    fn a_removal_refused_for_anything_but_absence_still_fails_the_attempt() {
        let dir = library("begin-refused-for-another-reason");
        put_a_file_where_the_directory_would_be(dir.staging().path());

        let refused = match Staging::begin(Flow::Joining, dir) {
            Err(refused) => refused,
            Ok(_) => panic!("a refusal that is not absence must stop the attempt"),
        };

        assert!(
            matches!(
                refused,
                Error::Local(LocalIoError {
                    operation: LocalOperation::Removing,
                    ..
                })
            ),
            "the attempt must fail as the removal it was: {refused:?}"
        );
    }

    /// Where a Library of this name would be, under this binary's own state
    /// directory.
    fn library(name: &str) -> LibraryDir {
        state_dir();
        LibraryDir::resolve(name).expect("a fixture names a Library a directory can be called")
    }

    /// Puts an ordinary file where the staging directory would be.
    ///
    /// A removal of it is refused for a reason that is not absence — a path
    /// that is not a directory — which is what the two cases above need and
    /// what no amount of removing gets rid of.
    fn put_a_file_where_the_directory_would_be(path: &Path) {
        let _ = fs::remove_dir_all(path);
        fs::create_dir_all(
            path.parent()
                .expect("a staging directory sits under the directory Libraries are kept in"),
        )
        .expect("the directory Libraries are kept in must be there");
        fs::write(path, b"not a directory").expect("a fixture must be able to write a file");
    }
}
