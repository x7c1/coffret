//! Giving a mapped root the identity its mapping will expect of it
//! (spec: EP-13).
//!
//! Recording a mapping is the only thing in coffret that writes here. A scan, a
//! fetch, an upload, and a sync neither create the management area nor the
//! marker, nor rewrite one, nor change what a mapping expects: they only read
//! the marker and compare. So everything the marker's file ever has done to it
//! is in this module, and it is done conservatively — where nothing is there a
//! fresh identity is written, where a valid marker is there its identity is
//! adopted and the file left alone, and every other state is an error that
//! writes nothing at all.
//!
//! The descent is EP-8's, for the reason EP-13 restates: the root is opened as
//! the person named it and may pass through a symbolic link, because what that
//! path points at is their configuration to make; `.coffret` and then `root` are
//! opened *below that open handle* without following links, so a name swapped
//! under the walk cannot redirect a write outside the root.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::root_marker::{self, MANAGEMENT_AREA, MARKER_FILE};
use coffret_usecase::LocalOperation;

use crate::error::{Error, Result};
use crate::marker_record::MarkerRecord;
use crate::marker_request::MarkerRequest;

// The descent as far as the folder the marker stands in: the mapped root, and
// `.coffret` below it made where it is absent.
mod management_area;
use management_area::ManagementArea;

// The three things ever done to the marker's own file, one to a module.
mod read_marker;
mod replace_marker;
mod write_marker;

/// The identity `root` carries once this returns, and how it came to carry it.
///
/// `root` is the canonicalized local root the mapping is about to be recorded
/// against. No marker is written unless the whole of the sequence succeeds, and
/// a half-written one is taken away again, so an error here leaves the root
/// carrying no identity it did not carry before — which is what lets a person
/// fix whatever the message names and record the mapping again. The management
/// area is made before the identity going into it is drawn, so that much can be
/// left behind.
pub(super) fn register(
    root: &Path,
    request: MarkerRequest,
) -> Result<(RootMarkerId, MarkerRecord)> {
    let directory = management_area::open_root(root)?;
    match management_area::enter_or_make(&directory, root)? {
        // Nothing was there, so nothing of anybody's is being replaced — and a
        // new identity asked for is what this does anyway.
        ManagementArea::Made(area) => write_marker::write(&area, root),
        // The marker is read before anything else is decided, with a new
        // identity asked for as much as without: the flag replaces an identity
        // and does not repair a broken management area, so a marker that names
        // none is a refusal either way.
        ManagementArea::Found(area) => match (read_marker::read(&area, root)?, request) {
            (found, MarkerRequest::AdoptWhatIsThere) => Ok((found, MarkerRecord::Adopted)),
            (_, MarkerRequest::IssueANewIdentity) => {
                replace_marker::replace(&area, root).map(|id| (id, MarkerRecord::Reset))
            }
        },
    }
}

/// Writes an identity into an open file and flushes it, so that what the rename
/// publishes is bytes that have reached the disk.
fn fill(mut file: File, id: &RootMarkerId, root: &Path) -> Result<()> {
    file.write_all(&root_marker::spell(id))
        .map_err(Error::local(LocalOperation::Writing, marker_path(root)))?;
    file.sync_all()
        .map_err(Error::local(LocalOperation::Flushing, marker_path(root)))
}

/// A fresh identity, or the refusal that says none could be drawn.
fn draw(root: &Path) -> Result<RootMarkerId> {
    RootMarkerId::generate().map_err(|cause| Error::RootMarkerNotDrawn {
        root: root.to_path_buf(),
        cause,
    })
}

/// The management area's path, for a refusal that names a file rather than a
/// state.
fn area_path(root: &Path) -> PathBuf {
    root.join(MANAGEMENT_AREA)
}

/// The marker's own path, for the same.
fn marker_path(root: &Path) -> PathBuf {
    area_path(root).join(MARKER_FILE)
}
