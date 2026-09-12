use coffret_model::EntryPath;
use coffret_usecase::fetch::{local_place_for, FetchError};
use coffret_usecase::{root_marker, scratch};

use super::IncomingFile;
use crate::error::{Error, Result};
use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Opens the file at one Entry Path for writing, in the folder this device
    /// maps that part of the Library into (spec: EP-9).
    ///
    /// The path is where the file will stand in the Library once a sync has
    /// carried it in, and where it goes on disk is the mappings' answer — the
    /// same translation a fetch makes before placing an Entry, so a file added
    /// here and the same file fetched onto another device land in the folder each
    /// device chose for that subtree.
    ///
    /// Nothing is written by this call and nothing is decided by it beyond the
    /// path: the bytes go through [`IncomingFile`] and the file exists only when
    /// [`keep`](IncomingFile::keep) renames it into place.
    ///
    /// # Errors
    ///
    /// [`Error::Fetch`](crate::Error::Fetch) carrying `UnmappedEntryPath` where
    /// no mapping of this device reaches the path — nowhere on this device stands
    /// for that part of the Library, so there is nowhere to put the file — and
    /// `UnmaterializablePath` where a mapping does reach it and no file here may
    /// stand for it (spec: EP-2, EP-4).
    ///
    /// A path whose folders on this device are not folders is among the second,
    /// and it is the reason the destination is descended to here rather than
    /// joined: a component that is a symbolic link — pointing out of the mapped
    /// root or back inside it — is refused, because a file written through one
    /// would land somewhere the mappings never named (spec: EP-4, EP-11).
    ///
    /// The same `Error::Fetch` carrying `ReservedComponent` where a component of
    /// the path is one coffret keeps for itself inside a mapped folder: the
    /// scratch prefix a half-written file is called by
    /// ([`scratch`](coffret_usecase::scratch), spec: EP-11), or the device's own
    /// management area ([`root_marker`](coffret_usecase::root_marker),
    /// spec: EP-14). Both are names a scan passes over, so a file written under
    /// either would sit in a mapped folder that no sync will ever carry in —
    /// visible, the person's own, and permanently outside the Library — and one
    /// written at the management area's marker would take the root's identity
    /// away from it (spec: EP-13). Refusing the name says so at the moment it can
    /// still be changed, rather than accepting the file and quietly never backing
    /// it up. The refusal names the component, because that is the part of the
    /// path there is anything to do about (spec: EP-4).
    ///
    /// The same `Error::Fetch` carrying `Index` where the mappings could not be
    /// read at all, which is neither verdict about the path — nothing was
    /// decided, so nothing is refused.
    ///
    /// `Local` where the folders above the file could not be made, or the
    /// temporary file could not be created.
    ///
    /// [`Error::RootRefused`](crate::Error::RootRefused) where the mapped root
    /// is not the folder the mapping was recorded against. A fetch reports such
    /// a mapping and carries on with the device's others; this device is placing
    /// the one file it was handed and has no other mapping to go on with, so the
    /// request fails as a whole — and a caller handed several at once, as one
    /// upload's files are, has that same nothing to go on with for every one of
    /// them this mapping reaches. Nothing was written: only recording that
    /// mapping again settles which folder it is (spec: EP-11, EP-13). The
    /// refusal names the mapping — its Library-side prefix, or the Library root
    /// where it stands for that — so the gesture has one to be aimed at on a
    /// device that has more than one.
    pub async fn receive_file(&self, path: &EntryPath) -> Result<IncomingFile> {
        // One gate for both reservations, because they are one question: is any
        // name in this path coffret's own rather than the person's? Asked before
        // the mappings are read, since the answer is the path's alone.
        if let Some(component) = reserved(path) {
            return Err(FetchError::ReservedComponent {
                path: path.clone(),
                component: component.to_owned(),
            }
            .into());
        }
        let place = local_place_for(self.index.as_ref(), path).await?;
        let directory = place
            .descend(self.local_fs.as_ref())
            .await
            .map_err(|refused| Error::descent(refused, place.prefix(), path))?;
        IncomingFile::open(path.clone(), place.prefix().cloned(), directory).await
    }
}

/// The first component of `path` coffret keeps for itself, where there is one.
///
/// Every component is asked about, not only the topmost: a name is reserved at
/// any depth, so a check that looked at the top alone would walk straight past
/// `albums/.coffret`.
fn reserved(path: &EntryPath) -> Option<&str> {
    path.as_str().split('/').find(|component| {
        scratch::is_scratch(component) || root_marker::is_management_area(component)
    })
}
