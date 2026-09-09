use crate::device_state::Mapping;
use crate::local_error::LocalError;
use crate::local_scan::walked::RootState;
use crate::mapped_roots::MappedRoots;
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
/// The root is probed rather than listed first, and the probe follows links,
/// which is what listing the folder would resolve anyway — and unlike the
/// entries below it, which are stated with links unfollowed (spec: EP-8).
///
/// Only absence is a verdict, and it is the capability that says so: a mapped
/// root that is a regular file, or one the process may not stat at all, still
/// fails the run with the [`LocalError::Io`] the gateway reports, carrying the
/// same path and the same operating-system cause. Nothing here reads an error
/// kind to tell the two apart.
pub(super) async fn root_state(
    roots: &dyn MappedRoots,
    mapping: &Mapping,
) -> Result<RootState, LocalError> {
    let root = mapping.local_root.as_path();
    let Some(probe) = roots.probe_root(root).await? else {
        return Ok(RootState::Unavailable(RootUnavailable::Missing));
    };

    let Some(current) = probe.identity else {
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
    //
    // The test is the plainest possible one — not "no regular file", and not
    // "nothing a scan would back up". An unmounted mount point is an empty
    // directory, and widening it would start guessing about folders that hold
    // something the walk passes over.
    match roots.list_folder(root, None).await? {
        // The root was there a moment ago and is not there now, which is the
        // missing-root verdict arriving late rather than a reason to fail — and
        // it is that verdict and not the identity mismatch, because the reason
        // travels to the caller and what happened is that the root went away.
        None => Ok(RootState::Unavailable(RootUnavailable::Missing)),
        Some(entries) if entries.is_empty() => {
            Ok(RootState::Unavailable(RootUnavailable::AnotherFilesystem))
        }
        Some(_) => Ok(RootState::Stamp(current)),
    }
}
