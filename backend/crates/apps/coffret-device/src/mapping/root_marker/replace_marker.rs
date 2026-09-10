use std::fs::File;
use std::os::fd::OwnedFd;
use std::path::Path;

use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::root_marker::MARKER_FILE;
use coffret_usecase::LocalOperation;
use rustix::fs::{AtFlags, Mode, OFlags};

use super::{draw, fill, marker_path};
use crate::error::{Error, Result};

/// Puts a freshly drawn identity in place of the one a valid marker carries.
///
/// Written under a name of its own and renamed over the marker, so that no
/// reader ever opens a marker that is half a line — the same discipline a fetch
/// publishes a file with (spec: EP-11). It is not one of that fetch's
/// scratches: a scratch is a half-written file inside a folder a scan walks and
/// carries the prefix reserved so the scan steps over it, while this one stands
/// inside the management area, which a scan never enters at all (spec: EP-14).
/// It is named after the identity going into it, so two runs resetting one root
/// write two files rather than into one.
pub(super) fn replace(area: &OwnedFd, root: &Path) -> Result<RootMarkerId> {
    let id = draw(root)?;
    let new_marker = format!("{MARKER_FILE}.{id}.new");

    let opened = rustix::fs::openat(
        area,
        &new_marker,
        OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_bits_truncate(0o666),
    )
    .map_err(|cause| Error::local(LocalOperation::Creating, marker_path(root))(cause.into()))?;

    let published = fill(File::from(opened), &id, root).and_then(|()| {
        rustix::fs::renameat(area, new_marker.as_str(), area, MARKER_FILE).map_err(|cause| {
            Error::local(LocalOperation::Renaming, marker_path(root))(cause.into())
        })
    });
    if published.is_err() {
        // The marker still holds the identity it held, so what this leaves
        // behind is a half-written file nothing points at. It goes, and the
        // refusal is what the caller hears about.
        let _ = rustix::fs::unlinkat(area, new_marker.as_str(), AtFlags::empty());
    }
    published.map(|()| id)
}
