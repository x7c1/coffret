use std::fs::File;
use std::io::Read;
use std::os::fd::OwnedFd;
use std::path::Path;

use coffret_usecase::device_state::RootMarkerId;
use coffret_usecase::root_marker::{self, MARKER_FILE};
use coffret_usecase::LocalOperation;
use rustix::fs::{Mode, OFlags};
use rustix::io::Errno;

use super::marker_path;
use crate::error::{Error, Result};

/// The identity the marker standing in an existing management area names.
///
/// Every way of not being one is an error and none of them is repaired: a
/// missing marker is the interrupted registration EP-13 names, a link or a
/// device or a folder at the name is not the file the rule is about, and content
/// that parses as no identity is not one this device may adopt.
pub(super) fn read(area: &OwnedFd, root: &Path) -> Result<RootMarkerId> {
    let opened = rustix::fs::openat(
        area,
        MARKER_FILE,
        // `O_NONBLOCK` so that a named pipe standing at the name answers the
        // open instead of holding it: what it is gets decided below, and an open
        // that never returns decides nothing.
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
        Mode::empty(),
    );
    let file = match opened {
        Ok(file) => File::from(file),
        Err(Errno::NOENT) => {
            return Err(Error::ManagementAreaIncomplete {
                root: root.to_path_buf(),
            })
        }
        // The link the open turned away, in either spelling: `O_NOFOLLOW`
        // reports `ELOOP` — `EMLINK` where the BSDs spell it that way. An
        // identity read through a link would be whatever it points at rather
        // than this root's, and the placement side makes the same verdict of the
        // same two errnos.
        Err(Errno::LOOP | Errno::MLINK) => {
            return Err(Error::MarkerNotARegularFile {
                root: root.to_path_buf(),
            })
        }
        Err(cause) => {
            return Err(Error::local(LocalOperation::Reading, marker_path(root))(
                cause.into(),
            ))
        }
    };

    // `O_NOFOLLOW` turned a symbolic link away and nothing else: a folder, a
    // device, a socket, and a pipe all open. What the handle is is asked of the
    // handle, so a name swapped after the open cannot change the answer.
    let regular = file
        .metadata()
        .map_err(Error::local(LocalOperation::Stating, marker_path(root)))?
        .is_file();
    if !regular {
        return Err(Error::MarkerNotARegularFile {
            root: root.to_path_buf(),
        });
    }

    // One byte past the cap, which is the least that shows a file running on
    // past it: the rest of such a file is never read, so a root pointed at an
    // enormous or an endless one costs this read the cap and a byte and no more
    // (spec: EP-13).
    let mut content = Vec::with_capacity(root_marker::MAX_LEN + 1);
    file.take((root_marker::MAX_LEN + 1) as u64)
        .read_to_end(&mut content)
        .map_err(Error::local(LocalOperation::Reading, marker_path(root)))?;

    root_marker::parse(&content).map_err(|cause| Error::MarkerMalformed {
        root: root.to_path_buf(),
        cause,
    })
}
