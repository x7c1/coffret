use std::io;
use std::path::Path;

use tokio::fs;

use crate::device_state::{Mapping, RootIdentity};
use crate::local_error::LocalError;
use crate::local_operation::LocalOperation;
use crate::local_scan::walked::RootState;
use crate::unavailable_root::RootUnavailable;

/// What one mapped root is, asked before anything under it is read
/// (spec: EP-12).
///
/// The two shapes an unavailable root takes are answered here and nowhere else,
/// because both are questions about the root *itself* and the walk below has no
/// way to ask them: its stack carries no marker for "this is the root", so the
/// answer that is right for a subdirectory that vanished mid-walk — pass over it
/// and carry on — would otherwise also answer for a root that was never opened
/// at all. That is how a mapped root that is not there came to read as a folder
/// holding nothing, and every Entry under it as deleted.
///
/// The root is stated with [`fs::metadata`], following links, which is what
/// [`fs::read_dir`] already does to the root — and unlike the
/// `symlink_metadata` the entries below it are stated with (spec: EP-8).
///
/// Only absence is a verdict. A mapped root that is a regular file, or one the
/// process may not stat at all, still fails the run, and with the same
/// [`LocalError::Io`] carrying the same path and the same operating-system cause
/// the walk would have carried. For the root it may not stat, what moves is
/// which [`LocalOperation`] that value names: the root is stated before it is
/// listed now, so the refusal the walk used to report as a listing is reported
/// as a stat, which is the call that was actually refused. A root it can stat
/// but not read fails at the listing instead, exactly as before.
pub(super) async fn root_state(mapping: &Mapping) -> Result<RootState, LocalError> {
    let root = mapping.local_root.as_path();
    let metadata = match fs::metadata(root).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RootState::Unavailable(RootUnavailable::Missing))
        }
        Err(cause) => return Err(LocalError::io(LocalOperation::Stating, root, cause)),
    };

    let Some(current) = identity_of(&metadata) else {
        // A platform that can say nothing about the filesystem under a path
        // records nothing, and such a mapping is guarded by the missing-root
        // check alone.
        return Ok(RootState::Available);
    };
    let Some(recorded) = mapping.root_identity.as_ref() else {
        // The first scan of a new mapping records what it saw. Everything the
        // comparison below can conclude rests on this stamp having been taken
        // while the root was really there.
        return Ok(RootState::Stamp(current));
    };
    if *recorded == current {
        return Ok(RootState::Available);
    }

    // The identity moved, and what that means depends entirely on whether the
    // root holds anything. An unmounted mount point is an ordinary empty
    // directory; a root that holds files is a folder whose device number was
    // renumbered by a reboot or a remount, and calling that unavailable would
    // silently stop backing the folder up. So the asymmetry is deliberate: empty
    // is a verdict, non-empty is a re-stamp (spec: EP-12).
    match list_root(root).await? {
        RootListing::Gone => Ok(RootState::Unavailable(RootUnavailable::Missing)),
        RootListing::Empty => Ok(RootState::Unavailable(RootUnavailable::AnotherFilesystem)),
        RootListing::Holding => Ok(RootState::Stamp(current)),
    }
}

/// What listing the root turned up, asked only where what is inside it decides
/// the verdict.
enum RootListing {
    /// The root was there to be stated and is not there to be listed.
    Gone,
    /// The root holds no directory entry at all.
    Empty,
    /// The root holds something.
    Holding,
}

/// Whether the root holds a directory entry at all, or has gone away since it
/// was stated.
///
/// Deliberately the plainest possible test — not "no regular file", and not
/// "nothing a scan would back up". An unmounted mount point is an empty
/// directory, and widening the test would start guessing about folders that hold
/// something the walk passes over.
async fn list_root(root: &Path) -> Result<RootListing, LocalError> {
    let mut listing = match fs::read_dir(root).await {
        Ok(listing) => listing,
        // The root was there a moment ago and is not there now, which is the
        // missing-root verdict arriving late rather than a reason to fail — and
        // it is that verdict and not the identity mismatch, because the reason
        // travels to the caller and what happened is that the root went away.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(RootListing::Gone),
        Err(cause) => return Err(LocalError::io(LocalOperation::Listing, root, cause)),
    };
    let first = listing
        .next_entry()
        .await
        .map_err(|cause| LocalError::io(LocalOperation::Listing, root, cause))?;
    Ok(if first.is_some() {
        RootListing::Holding
    } else {
        RootListing::Empty
    })
}

/// What this platform can say about the filesystem one folder stands on, or
/// `None` where it can say nothing (spec: EP-12).
///
/// The `unix-dev:` tag is not decoration: it is what keeps a value one platform
/// recorded from ever comparing equal to a value another platform's form
/// happened to spell the same way, which matters because the comparison is what
/// decides whether deletion inference runs.
#[cfg(unix)]
fn identity_of(metadata: &std::fs::Metadata) -> Option<RootIdentity> {
    use std::os::unix::fs::MetadataExt;

    Some(RootIdentity::new(format!("unix-dev:{}", metadata.dev())))
}

#[cfg(not(unix))]
fn identity_of(_metadata: &std::fs::Metadata) -> Option<RootIdentity> {
    None
}
