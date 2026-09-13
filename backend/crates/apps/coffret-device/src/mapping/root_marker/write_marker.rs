use std::fs::File;
use std::os::fd::OwnedFd;
use std::path::Path;

use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::root_marker::MARKER_FILE;
use coffret_usecase::LocalOperation;
use rustix::fs::{AtFlags, Mode, OFlags};
use rustix::io::Errno;

use super::{draw, fill, marker_path, read_marker};
use crate::error::{Error, Result};
use crate::marker_record::MarkerRecord;

/// Writes a freshly drawn identity into a management area that holds no marker.
///
/// Create-exclusive, so a registration racing this one cannot have its marker
/// overwritten: whichever call creates the file has written the root's identity,
/// and the other adopts it rather than replacing it. Which of the two this call
/// turned out to be is what comes back with the identity, because it is what the
/// person is told.
///
/// Where the writing fails the file goes again, so that the root is left
/// carrying no identity rather than a marker that names none — which is a state
/// ordinary operation never gets a root out of.
pub(super) fn write(area: &OwnedFd, root: &Path) -> Result<(RootMarkerId, MarkerRecord)> {
    let id = draw(root)?;
    match rustix::fs::openat(
        area,
        MARKER_FILE,
        OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::from_bits_truncate(0o666),
    ) {
        Ok(file) => {
            let written = fill(File::from(file), &id, root);
            if written.is_err() {
                // What this call created is empty or half a line, and a marker
                // that names no identity is refused by every later run —
                // including one asking for a new identity, since the flag
                // replaces an identity rather than repairing a marker. The file
                // is this call's own, created where nothing was, so taking it
                // away leaves the root carrying no identity and the refusal is
                // the whole of what the caller hears about.
                let _ = rustix::fs::unlinkat(area, MARKER_FILE, AtFlags::empty());
            }
            written.map(|()| (id, MarkerRecord::Written))
        }
        // Another registration created the marker between this one making the
        // folder and writing into it. Its identity is the root's, and adopting
        // it is what every other reader of an existing marker does.
        Err(Errno::EXIST) => {
            read_marker::read(area, root).map(|found| (found, MarkerRecord::Adopted))
        }
        Err(cause) => Err(Error::local(LocalOperation::Creating, marker_path(root))(
            cause.into(),
        )),
    }
}
