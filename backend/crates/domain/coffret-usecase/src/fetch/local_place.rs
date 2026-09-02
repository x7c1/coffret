use std::io;
use std::path::PathBuf;

use crate::fetch::confined_dir::{self, ConfinedDir};
use crate::fetch::descent_error::DescentError;
use crate::fetch::standing::Standing;
use crate::local_operation::LocalOperation;

/// Where one Entry Path's file belongs on this device (spec: EP-9).
///
/// Not a path but the two things a path is made of here: the mapped root the
/// file stands under, and the Entry Path's components below the mapping's
/// prefix. Keeping them apart is what lets a writer descend the components one
/// at a time — the root is the place the person configured, and everything below
/// it has to be a real folder of that root before any byte is written
/// (spec: EP-4, EP-11).
///
/// [`to_path_buf`](Self::to_path_buf) is the joined path, and it is for reading
/// and for reporting: a caller that already holds a file may open it by name,
/// and an error may say which file it is about. What a *writer* does is
/// [`descend`](Self::descend), which never hands a joined path to the operating
/// system at all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPlace {
    /// The mapping's local root.
    root: PathBuf,
    /// The components below the mapping's prefix, the last being the file's own
    /// name. Never empty.
    relative: Vec<String>,
}

impl LocalPlace {
    /// The place a mapping's local root and an Entry Path's remaining
    /// components make.
    ///
    /// Only [`translate`](super::translate) builds one, because EP-9 has one
    /// implementation: a second reading of the mappings is what would let a file
    /// be written somewhere a fetch would never look for it.
    pub(super) fn new(root: PathBuf, relative: Vec<String>) -> Self {
        debug_assert!(
            !relative.is_empty(),
            "a place under a mapped root names at least the file itself",
        );
        Self { root, relative }
    }

    /// The local path the two halves join to.
    ///
    /// What a reader opens and what an error names. Never what a writer walks:
    /// joining the components is exactly the step that would let the operating
    /// system follow a symbolic link on the way down.
    pub fn to_path_buf(&self) -> PathBuf {
        let mut joined = self.root.clone();
        joined.extend(&self.relative);
        joined
    }

    /// Opens the folder the file belongs in, making the folders above it and
    /// refusing to pass through anything that is not a real folder of the mapped
    /// root.
    ///
    /// This is the one way into a mapped folder for anything that writes: both
    /// the fetch placing a verified Entry and the explorer taking a dropped file
    /// go through it, so the fence is one piece of code rather than two readings
    /// of one rule (spec: EP-4, EP-11).
    ///
    /// # Errors
    ///
    /// [`DescentError::Blocked`] where a component on the way down is a symbolic
    /// link or is not a folder — the Entry Path cannot be materialized on this
    /// device, whatever the link points at — and [`DescentError::Io`] where the
    /// operating system refused for any other reason.
    pub async fn descend(&self) -> Result<ConfinedDir, DescentError> {
        let root = self.root.clone();
        let relative = self.relative.clone();
        self.blocking(LocalOperation::Creating, move || {
            confined_dir::descend(&root, &relative)
        })
        .await
    }

    /// What stands at the file's path now, reached the same confined way.
    ///
    /// `None` where nothing is there, which includes a folder on the way that
    /// does not exist yet: an empty place is an empty place however few of the
    /// folders above it have been made. A symbolic link anywhere on the way down
    /// is refused rather than answered for, because what a writer would find
    /// past it is not this device's mapped folder.
    ///
    /// Internal to the fetch, which is the only thing that has to decide whether
    /// it may write at a path (spec: EP-11).
    ///
    /// # Errors
    ///
    /// The two [`descend`](Self::descend) reports, for the same two reasons.
    pub(super) async fn look(&self) -> Result<Option<Standing>, DescentError> {
        let root = self.root.clone();
        let relative = self.relative.clone();
        self.blocking(
            LocalOperation::Stating,
            move || match confined_dir::look_up(&root, &relative)? {
                Some(directory) => directory.look(),
                None => Ok(None),
            },
        )
        .await
    }

    /// Runs one descent off the runtime's threads.
    ///
    /// The syscalls are blocking and there is no asynchronous `openat`:
    /// `tokio::fs` offers path-based calls alone, which are precisely the ones
    /// this type exists not to make. A descent is a handful of them and an
    /// unbounded number for a deep Entry Path, so it goes where blocking work
    /// goes rather than being called inline.
    async fn blocking<T: Send + 'static>(
        &self,
        operation: LocalOperation,
        work: impl FnOnce() -> Result<T, DescentError> + Send + 'static,
    ) -> Result<T, DescentError> {
        match tokio::task::spawn_blocking(work).await {
            Ok(answer) => answer,
            Err(joined) => Err(DescentError::Io {
                operation,
                path: self.to_path_buf(),
                cause: io::Error::other(joined),
            }),
        }
    }
}
