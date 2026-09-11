use crate::device_state::Mapping;
use crate::folder_entry::FolderEntry;
use crate::local_error::LocalError;
use crate::local_scan::walked::RootState;
use crate::mapped_roots::MappedRoots;
use crate::root_marker;
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
    // "nothing a scan would back up" — with the single exception the rule names
    // itself: the device's own management area is not anybody's content, so a
    // root holding nothing else holds nothing here (spec: EP-13, EP-14).
    // Counting it would be counting coffret's own bookkeeping as a folder's
    // contents, and since recording a mapping writes it into every mapped root,
    // that would leave *no* root reading as empty and the whole protection of
    // deletion inference gone. Widening it any further would start guessing
    // about folders that hold something the walk passes over.
    match roots.list_folder(root, None).await? {
        // The root was there a moment ago and is not there now, which is the
        // missing-root verdict arriving late rather than a reason to fail — and
        // it is that verdict and not the identity mismatch, because the reason
        // travels to the caller and what happened is that the root went away.
        None => Ok(RootState::Unavailable(RootUnavailable::Missing)),
        Some(entries) if holds_nothing(&entries) => {
            Ok(RootState::Unavailable(RootUnavailable::AnotherFilesystem))
        }
        Some(_) => Ok(RootState::Stamp(current)),
    }
}

/// Whether a root's listing is nothing but the device's own (spec: EP-12).
///
/// The comparison looks past `.coffret/`, and past it alone: every other name is
/// something a person or another program put there, whether or not a scan would
/// back it up. A name this device cannot read as text is one of those too — it is
/// not the reserved name, so it counts as content the way the walk's own refusal
/// of it does (spec: EP-1, EP-14).
fn holds_nothing(entries: &[FolderEntry]) -> bool {
    entries.iter().all(|entry| {
        entry
            .name
            .to_str()
            .is_some_and(root_marker::is_management_area)
    })
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::device_state::RootIdentity;
    use crate::in_memory_fs::InMemoryFs;

    /// Where a case's mapped folder stands in the fake, which is any path at
    /// all: nothing here is on a disk.
    const ROOT: &str = "/folder";

    /// A mapping stamped with a filesystem the fake does not report for the
    /// root, which is what puts the emptiness question (spec: EP-12).
    fn moved() -> Mapping {
        Mapping::new(None, PathBuf::from(ROOT)).stamped(RootIdentity::new("another-filesystem"))
    }

    // EP-12: recording a mapping writes `.coffret/` into the root it records
    // (spec: EP-13), so a root holding only that is exactly what an unplugged
    // disk leaves behind — and a comparison that counted the management area
    // would read every such root as a folder still holding files, re-stamp it,
    // and infer the deletion of everything the disk carried.
    #[tokio::test]
    async fn a_root_holding_only_the_management_area_still_reads_as_empty() {
        let fs = InMemoryFs::new();
        let root = Path::new(ROOT);
        fs.create_dir(&root.join(root_marker::MANAGEMENT_AREA));

        let state = root_state(&fs, &moved())
            .await
            .expect("asking what a root is must succeed");
        assert!(
            matches!(
                state,
                RootState::Unavailable(RootUnavailable::AnotherFilesystem),
            ),
            "a root holding nothing but coffret's own folder holds nothing",
        );

        // And the other half of the asymmetry is untouched: one file of the
        // person's own beside it, and the root is a folder whose filesystem
        // moved rather than one that went away.
        fs.write_file(&root.join("a.jpg"), b"some bytes");
        let state = root_state(&fs, &moved())
            .await
            .expect("asking what a root is must succeed");
        assert!(
            matches!(state, RootState::Stamp(_)),
            "a root that holds a file is re-stamped, management area or no",
        );
    }
}
