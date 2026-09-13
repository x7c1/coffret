//! Where a local writer puts a file on this device, and the walk that reaches
//! its folder without passing through a symbolic link (spec: EP-4, EP-11).
//!
//! This is what "placing a file inside a mapped folder" means once EP-4 and
//! EP-11 are taken seriously. An Entry Path comes from another enrolled device
//! and says nothing about the shape of *this* device's disk: the same
//! `link/authorized_keys` is an ordinary folder and a file on the device that
//! committed it, and may be a symbolic link out of the mapped root here. A
//! writer that joined the components onto the root and handed the string to the
//! operating system would follow that link and write where the Library never
//! pointed. So the descent walks the components one at a time and refuses
//! anything that is not a real directory, and what it hands back is the open
//! directory rather than a path.
//!
//! Every write is then made *relative to that handle* — the scratch, the
//! rename that publishes it, the removal that cleans it up — so no answer can go
//! stale between the descent and the write. A path rebuilt from the root and
//! handed back to the operating system would ask the question again, and a name
//! that became a symbolic link in between would be followed on that second
//! asking.
//!
//! The mapped root itself is opened as the path the user configured it as. What
//! that path points at is theirs to choose (spec: EP-9); what is under it is the
//! Library's, and that is what is walked component by component.
//!
//! Which folder that turns out to be is the other question a *write* asks, and it
//! is asked here rather than anywhere else for the same reason the walk is: the
//! marker standing in the root is read below the handle the descent has just
//! opened and the placement then writes through, so nothing between the question
//! and the write can change the answer (spec: EP-13). A read asks nothing of the
//! kind — it places nothing.
//!
//! Unix only, deliberately, and the one part of this crate that is. The
//! primitives are `openat`, `mkdirat`, `renameat`, and `unlinkat` with
//! `O_NOFOLLOW` and `O_DIRECTORY`, which is what expresses "descend one name
//! without following a link" — coffret runs on Linux and macOS, and a
//! portability layer over platforms it does not run on would be a second answer
//! to a question that has one.

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::{
    DescentError, Destination, Destinations, LocalIoError, LocalOperation, Standing,
};
use rustix::io::Errno;

use crate::unix_destinations::unix_destination::UnixDestination;
use crate::unix_fs::UnixFs;

// The walk itself, and what one stat of the file's own name found.
mod descent;

mod look;

// The open folder every call is made against, and what the three steps of a
// placement hold while it is going on.
mod open_folder;

mod unix_destination;

mod unix_flushed_file;

mod unix_scratch_file;

// What the descent asks of the root it has just opened, before it descends
// anything below it (spec: EP-13).
mod vouch;

#[async_trait]
impl Destinations for UnixFs {
    async fn reach(
        &self,
        root: &Path,
        expected: Option<&RootMarkerId>,
        components: &[String],
    ) -> Result<Box<dyn Destination>, DescentError> {
        let owned = root.to_path_buf();
        // Copied rather than borrowed, because the descent runs off this thread
        // and an identity is eight bytes.
        let expected = expected.copied();
        let components = components.to_vec();
        let folder = blocking(LocalOperation::Creating, root, move || {
            descent::descend(&owned, expected.as_ref(), &components)
        })
        .await?;
        Ok(Box::new(UnixDestination::new(Arc::new(folder))))
    }

    async fn look_up(
        &self,
        root: &Path,
        components: &[String],
    ) -> Result<Option<Standing>, DescentError> {
        let owned = root.to_path_buf();
        let components = components.to_vec();
        blocking(
            LocalOperation::Stating,
            root,
            move || match descent::look_up(&owned, &components)? {
                Some(folder) => look::standing(&folder),
                None => Ok(None),
            },
        )
        .await
    }
}

/// Runs one descent off the runtime's threads.
///
/// The syscalls are blocking and there is no asynchronous `openat`: the
/// runtime's own file API offers path-based calls alone, which are precisely the
/// ones this walk exists not to make. A descent is a handful of them and an
/// unbounded number for a deep Entry Path, so it goes where blocking work goes
/// rather than being called inline.
///
/// A task that could not be joined is reported against the mapped root, because
/// that is the one path the caller named and the walk may not have reached any
/// other.
async fn blocking<T: Send + 'static>(
    operation: LocalOperation,
    root: &Path,
    work: impl FnOnce() -> Result<T, DescentError> + Send + 'static,
) -> Result<T, DescentError> {
    match tokio::task::spawn_blocking(work).await {
        Ok(answer) => answer,
        Err(joined) => Err(DescentError::Io(LocalIoError::new(
            operation,
            root,
            std::io::Error::other(joined),
        ))),
    }
}

/// What one refused syscall means: a fence the descent met, or an I/O failure.
///
/// `ELOOP` and `ENOTDIR` are read as one verdict: the path cannot be
/// materialized here rather than that the disk went wrong. Which of the two a
/// kernel reports for a symbolic link depends on the platform and on whether
/// `O_DIRECTORY` was passed beside `O_NOFOLLOW` — Linux answers `ELOOP` either
/// way, macOS `ENOTDIR` once `O_DIRECTORY` is there — so reading them as one is
/// what keeps that difference out of the walk. Every refused step of the descent
/// is read here, because deciding a verdict from an errno is exactly what the
/// capability exists to keep out of the layer above; the root vouching beside
/// this matches the same pair with arms of its own rather than through here, so
/// what a port to a platform that answers something else has to settle is the
/// list in the crate documentation rather than this function alone.
fn refusal(at: &Path, operation: LocalOperation, cause: Errno) -> DescentError {
    if cause == Errno::LOOP || cause == Errno::NOTDIR {
        return DescentError::Blocked {
            stopped_at: at.to_path_buf(),
        };
    }
    DescentError::Io(LocalIoError::new(
        operation,
        at,
        std::io::Error::from(cause),
    ))
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;

    use rustix::fs::{Mode, OFlags};

    use super::*;

    // What this host reports for a symbolic link standing at a reserved name,
    // held against the reading `refusal` makes of it: the errnos it compares are
    // what a host answers rather than what a manual documents. On a platform
    // whose kernel answers a third value this fails, which is the compile-time
    // gate's message arriving from the other side (spec: EP-13, EP-14).
    //
    // What it drives is `refusal`, and `refusal` is not the only reader: the
    // root vouching beside it matches the same errnos with arms of its own, and
    // so do the device's `root_marker` opens. A port that taught `refusal` a new
    // errno and left those alone would still pass here, so it is the porting
    // note in this crate's documentation, not this test, that is the list of
    // what a port has to change. Those arms are driven against a real directory
    // instead: the shared conformance suite's management-area case stands a
    // planted "other" — a symbolic link, in this crate's fixture — at the
    // reserved name, and `fetch_confinement` puts one at the marker.
    #[test]
    fn a_symbolic_link_at_a_reserved_name_is_read_as_blocked() {
        let folders = tempfile::tempdir().expect("a temporary directory must be available");
        let folder = folders.path().join("a-folder");
        std::fs::create_dir(&folder).expect("the folder must be creatable");
        let file = folders.path().join("a-file");
        std::fs::write(&file, b"0011223344556677\n").expect("the file must be creatable");

        let to_a_folder = folders.path().join("link-to-a-folder");
        symlink(&folder, &to_a_folder).expect("a symbolic link must be creatable");
        let to_a_file = folders.path().join("link-to-a-file");
        symlink(&file, &to_a_file).expect("a symbolic link must be creatable");

        // Each link points at the kind of thing the name it stands for is meant
        // to hold, so nothing but `O_NOFOLLOW` turns the open away, and the
        // flags are the ones the two sides use: the management area's descent,
        // and the marker's open.
        let opens = [
            (
                "a folder the descent would enter",
                &to_a_folder,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::DIRECTORY,
            ),
            (
                "the marker a root is identified by",
                &to_a_file,
                OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK,
            ),
        ];
        for (name, link, flags) in opens {
            let cause = rustix::fs::open(link.as_path(), flags, Mode::empty())
                .expect_err("`O_NOFOLLOW` must refuse to open a symbolic link");
            assert!(
                matches!(
                    refusal(link, LocalOperation::Stating, cause),
                    DescentError::Blocked { .. }
                ),
                "a symbolic link standing in for {name} reported {cause:?}, which \
                 this platform's kernel is entitled to, but `refusal` reads it as \
                 an I/O failure rather than as a fence. A port to this platform \
                 settles this errno here, at the root vouching beside this, and \
                 at the device's `root_marker` opens together — see this crate's \
                 documentation."
            );
        }
    }
}
