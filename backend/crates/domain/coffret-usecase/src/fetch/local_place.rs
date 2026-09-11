use std::path::PathBuf;

use crate::descent_error::DescentError;
use crate::destination::Destination;
use crate::destinations::Destinations;
use crate::device_state::{Mapping, RootMarkerId};
use crate::mapped_roots::MappedRoots;
use crate::source_reader::SourceReader;
use crate::standing::Standing;
use crate::{LocalIoError, MappedRelativeLocation};

/// Where one Entry Path's file belongs on this device (spec: EP-9).
///
/// Not a path but the two things a path is made of here: the mapped root the
/// file stands under, and the Entry Path's components below the mapping's
/// prefix. Keeping them apart is what lets a writer descend the components one
/// at a time — the root is the place the person configured, and everything below
/// it has to be a real folder of that root before any byte is written
/// (spec: EP-4, EP-11).
///
/// [`to_path_buf`](Self::to_path_buf) is the joined path for reporting and
/// collision checks. Reads use [`open`](Self::open), and writes use
/// [`descend`](Self::descend); both keep the two halves apart while the gateway
/// descends them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalPlace {
    /// The mapping's local root.
    root: PathBuf,
    /// The identity that mapping recorded for its root, or `None` where it
    /// records none (spec: EP-13).
    ///
    /// Carried here rather than looked up again where a write happens, for the
    /// reason the root and the components are carried together: the mapping that
    /// says *where* is the mapping that says *which folder*, and a second
    /// reading of the mappings could answer the two questions from different
    /// rows.
    expected: Option<RootMarkerId>,
    /// The components below the mapping's prefix, the last being the file's own
    /// name. Never empty.
    relative: MappedRelativeLocation,
}

impl LocalPlace {
    /// The place one mapping and an Entry Path's remaining components make.
    ///
    /// Only [`translate`](super::translate) builds one, because EP-9 has one
    /// implementation: a second reading of the mappings is what would let a file
    /// be written somewhere a fetch would never look for it.
    pub(super) fn new(mapping: &Mapping, relative: MappedRelativeLocation) -> Self {
        Self {
            root: mapping.local_root.clone(),
            expected: mapping.expected_root_id,
            relative,
        }
    }

    /// The local path the two halves join to.
    ///
    /// What an error names and translation uses for collision checks. Neither a
    /// reader nor a writer reaches the filesystem through this joined path.
    pub fn to_path_buf(&self) -> PathBuf {
        let mut joined = self.root.clone();
        joined.push(self.relative.to_path_buf());
        joined
    }

    /// Opens this mapped file without following a descendant symbolic link.
    pub async fn open(
        &self,
        roots: &dyn MappedRoots,
    ) -> Result<Box<dyn SourceReader>, LocalIoError> {
        roots.open_source(&self.root, &self.relative).await
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
    /// The walk itself is [`Destinations::reach`]'s — how a folder is reached
    /// without following a link is a filesystem's business and the gateway's to
    /// answer. What stays here is the shape of the question: the root apart from
    /// the components, which is what makes walking them possible at all.
    ///
    /// The mapping's expected identity travels with the call, because the
    /// capability holds the root's marker against it before it descends a single
    /// component and places nothing into a folder that is not the one the
    /// mapping was recorded against (spec: EP-13). Asked there rather than here
    /// so that the question and the write are made through one open handle.
    ///
    /// # Errors
    ///
    /// [`DescentError::Refused`] where the mapped root is not the folder the
    /// mapping expects, [`DescentError::Blocked`] where a component on the way
    /// down is a symbolic link or is not a folder — the Entry Path cannot be
    /// materialized on this device, whatever the link points at — and
    /// [`DescentError::Io`] where the operating system refused for any other
    /// reason.
    pub async fn descend(
        &self,
        destinations: &dyn Destinations,
    ) -> Result<Box<dyn Destination>, DescentError> {
        destinations
            .reach(
                &self.root,
                self.expected.as_ref(),
                &self
                    .relative
                    .text_components()
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
            )
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
    /// Public, with [`descend`](Self::descend), and for the same reason: the
    /// fetch asks it to decide whether it may write at a path (spec: EP-11),
    /// and the explorer that serves a file the Library holds no Entry at asks
    /// it to find out whether there is a file of this device's there. Two
    /// readings of the confined look would be two answers about one folder.
    ///
    /// # Errors
    ///
    /// The two [`descend`](Self::descend) reports, for the same two reasons.
    pub async fn look(
        &self,
        destinations: &dyn Destinations,
    ) -> Result<Option<Standing>, DescentError> {
        destinations
            .look_up(
                &self.root,
                &self
                    .relative
                    .text_components()
                    .map(str::to_owned)
                    .collect::<Vec<_>>(),
            )
            .await
    }
}
