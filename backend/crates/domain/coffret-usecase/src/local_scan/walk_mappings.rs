use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::Path;

use coffret_model::EntryPath;
use tokio::fs;

use crate::device_state::Mapping;
use crate::local_error::LocalError;
use crate::local_operation::LocalOperation;
use crate::local_scan::root_state::root_state;
use crate::local_scan::source_file::SourceFile;
use crate::local_scan::walked::{RootState, Walked, WalkedRoot};
use crate::local_times::{btime_of, mtime_of};
use crate::scratch;

/// Every regular file under every available mapping, and a verdict on each
/// mapped root.
///
/// A device may map a local root to the Library root and other local roots to
/// top-level components, and then the top-level mapping represents its subtree
/// while the Library-root mapping represents *the remainder* (spec: EP-9). So
/// the root walk stops at every top-level name another mapping stands for: a
/// folder called `albums` under the root-mapped folder is not a second spelling
/// of the `albums/` subtree, and walking it would either claim Entry Paths the
/// other mapping owns or collide with the files it holds.
///
/// Two local files reaching one Entry Path is then refused rather than
/// resolved: choosing one of them would back up whichever the walk happened to
/// reach second, and renaming one would invent a Library position the user never
/// asked for (spec: EP-4).
///
/// Each root is checked before anything under it is read (spec: EP-12), because
/// a root the device cannot vouch for is not a folder holding nothing. A root
/// that is not there, and one that is empty while standing on a filesystem the
/// mapping does not record — what an unmounted disk leaves behind — are reported
/// as [`RootState::Unavailable`] and not walked at all, so no caller can read
/// their emptiness as the user having emptied the folder. Every other mapping is
/// walked as usual, and a root that holds files on an unrecorded filesystem is
/// walked and its identity handed back to be re-stamped.
pub(crate) async fn walk_mappings(mappings: &[Mapping]) -> Result<Walked, LocalError> {
    // Every mapping's prefix, available or not. A top-level mapping still
    // represents its subtree while its drive is unplugged, so dropping its name
    // here would let the root mapping walk into the folder that stands where
    // that subtree belongs and commit Entry Paths the other mapping owns
    // (spec: EP-9, EP-12).
    //
    // These are held against names read off the disk, which this walk composes
    // before comparing them, and a prefix is an `EntryPath` and so already in
    // that same form (spec: EP-1). Both halves of every Entry Path assembled
    // below therefore come from one alphabet.
    let claimed: BTreeSet<&str> = mappings
        .iter()
        .filter_map(|mapping| mapping.prefix.as_ref())
        .map(EntryPath::as_str)
        .collect();

    let mut found: BTreeMap<EntryPath, SourceFile> = BTreeMap::new();
    let mut roots = Vec::with_capacity(mappings.len());
    for mapping in mappings {
        let state = root_state(mapping).await?;
        // An unavailable root is answered with its verdict and nothing else:
        // nothing under it is walked (spec: EP-12).
        if !matches!(state, RootState::Unavailable(_)) {
            // A top-level mapping stands for the whole of its own subtree, so
            // nothing is held back from its walk.
            let elsewhere = match mapping.prefix {
                Some(_) => BTreeSet::new(),
                None => claimed.clone(),
            };
            for source in walk(&mapping.local_root, mapping.prefix.as_ref(), &elsewhere).await? {
                if let Some(held) = found.insert(source.path.clone(), source) {
                    return Err(LocalError::PathCollision { path: held.path });
                }
            }
        }
        roots.push(WalkedRoot {
            mapping: mapping.clone(),
            state,
        });
    }
    Ok(Walked { found, roots })
}

/// Every regular file under one local root, at the Entry Paths the mapping
/// gives them (spec: EP-9).
///
/// Regular files only, and symbolic links are neither followed nor given an
/// Entry Path of their own — which is why every entry is stated with
/// `symlink_metadata` rather than `metadata` (spec: EP-8).
///
/// `prefix` and `elsewhere` are both in NFC, being an [`EntryPath`] and the
/// top-level components of others, and everything this walk reads off the disk
/// is put into NFC before it is compared with them or joined to them
/// (spec: EP-1).
///
/// `elsewhere` names the top-level components another mapping represents, which
/// this walk therefore does not enter (spec: EP-9). It is a top-level name and
/// so an Entry Path component, which is why nothing here says which one was
/// passed over.
async fn walk(
    root: &Path,
    prefix: Option<&EntryPath>,
    elsewhere: &BTreeSet<&str>,
) -> Result<Vec<SourceFile>, LocalError> {
    let mut found = Vec::new();
    // `None` is the root of this walk, which is no path at all: what stands
    // there is a name and not yet a position under it.
    let mut stack: Vec<(_, Option<EntryPath>)> = vec![(root.to_path_buf(), None)];

    while let Some((directory, relative)) = stack.pop() {
        let mut listing = match fs::read_dir(&directory).await {
            Ok(listing) => listing,
            // A directory that went away mid-walk holds no more files, which is
            // no reason to fail a run over the folders that are there. Only a
            // subdirectory ever reaches this: the root's own existence was
            // settled before the walk began (spec: EP-12).
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(cause) => return Err(LocalError::io(LocalOperation::Listing, directory, cause)),
        };

        while let Some(entry) = listing
            .next_entry()
            .await
            .map_err(|cause| LocalError::io(LocalOperation::Listing, directory.clone(), cause))?
        {
            let local_path = entry.path();
            let name = entry.file_name();
            let Some(text) = name.to_str() else {
                return Err(LocalError::UnrepresentableName { path: local_path });
            };
            // The name becomes an Entry Path component from here on, so this is
            // where the filesystem's spelling of it becomes the Library's and
            // where a name the Library cannot hold at all is turned away
            // (spec: EP-1, EP-2). A single component is an Entry Path in its own
            // right — the position of an Entry at the top of the Library — so
            // the constructor for text from outside is the one that reads it. It
            // is done per component and not on the assembled path because this
            // is the boundary — the same line that refuses a name no Unicode at
            // all can be made of — and because the join below owes no reading of
            // its own anyway.
            //
            // Nothing `read_dir` hands back is expected to fail this: a name
            // holds no `/`, is never empty, carries no NUL, and is never `.` or
            // `..`. It is answered rather than asserted all the same, and as
            // the same refusal a name that is not UTF-8 gets, for the reason
            // `LocalError::UnrepresentableName` gives.
            let Ok(name) = EntryPath::parse(text) else {
                return Err(LocalError::UnrepresentableName { path: local_path });
            };
            // A temporary file a fetch was killed in the middle of writing. It
            // is coffret's own scratch and not user data, so it is passed over
            // rather than committed as an Entry (spec: EP-11).
            if scratch::is_scratch(name.as_str()) {
                continue;
            }
            // At the top of this walk the name *is* the top-level component, so
            // this is where a subtree another mapping represents is left to it
            // (spec: EP-9).
            if relative.is_none() && elsewhere.contains(name.as_str()) {
                continue;
            }
            let below = match &relative {
                None => name,
                Some(relative) => relative.below(&name),
            };

            let metadata = match fs::symlink_metadata(&local_path).await {
                Ok(metadata) => metadata,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(cause) => {
                    return Err(LocalError::io(LocalOperation::Stating, local_path, cause))
                }
            };
            if metadata.is_dir() {
                stack.push((local_path, Some(below)));
            } else if metadata.is_file() {
                found.push(SourceFile {
                    path: entry_path(prefix, below),
                    local_path,
                    size: metadata.len(),
                    mtime: mtime_of(&metadata),
                    btime: btime_of(&metadata),
                });
            }
        }
    }
    Ok(found)
}

/// Where a file sits in the Library, given the mapping it was found under
/// (spec: EP-9).
///
/// Both halves are already what an Entry Path is — the prefix because it is one,
/// the relative path because the walk read each of its components as one — so
/// the join has nothing left to check and nothing left to compose
/// (spec: EP-1, EP-2).
fn entry_path(prefix: Option<&EntryPath>, relative: EntryPath) -> EntryPath {
    match prefix {
        Some(prefix) => prefix.below(&relative),
        None => relative,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entry_paths::entry_path as parsed;
    use crate::unavailable_root::RootUnavailable;

    // EP-11: a fetch writes its temporary file inside the very folder this walk
    // covers, so a run killed before the rename leaves one behind. Committing it
    // would put a partial file in the Library at an Entry Path the user never
    // asked for, which is why the two flows agree on a reserved prefix — and why
    // this is the one kind of name a scan passes over rather than reports
    // (spec: EP-1, EP-8).
    #[tokio::test]
    async fn a_temporary_file_a_fetch_left_is_not_a_source_file() {
        let root = tempfile::tempdir().expect("making a temporary directory must succeed");
        let container_id =
            coffret_format::generate_container_id().expect("the OS CSPRNG is available");
        let scratch_name = scratch::name(container_id);
        // A *folder* carrying the prefix, which no fetch makes but a user could.
        // The walk decides on the name before it stats what the name is, so the
        // whole subtree under it is passed over too — which is the width of the
        // trade EP-11 records.
        let scratch_folder = format!("{}album", scratch::PREFIX);

        for folder in ["below", &scratch_folder] {
            fs::create_dir_all(root.path().join(folder))
                .await
                .expect("making a folder must succeed");
        }
        for relative in [
            "a.jpg".to_owned(),
            "below/b.png".to_owned(),
            // At the top of the walk and below it, because the walk's other
            // reason to pass a name over applies only at the top (spec: EP-9).
            scratch_name.clone(),
            format!("below/{scratch_name}"),
            format!("{scratch_folder}/c.gif"),
        ] {
            fs::write(root.path().join(relative), b"some bytes")
                .await
                .expect("writing a file must succeed");
        }

        let walked = walk_mappings(&[Mapping {
            prefix: None,
            local_root: root.path().to_path_buf(),
            root_identity: None,
        }])
        .await
        .expect("walking a mapped folder must succeed");

        assert_eq!(
            walked.found.keys().cloned().collect::<Vec<_>>(),
            vec![parsed("a.jpg"), parsed("below/b.png")],
            "the user's files, and nothing under a name carrying the reserved prefix",
        );
    }

    // EP-12: the walk used to answer both with `continue` — a mapped root that
    // was never opened and a subdirectory that vanished mid-walk reached the same
    // `NotFound` arm, so a root that is not there came back as a folder holding
    // nothing and every Entry under it as deleted. The two answers are what the
    // whole rule rests on, and this is the one place the distinction can be
    // checked without a Library.
    #[tokio::test]
    async fn a_missing_root_is_not_an_empty_root() {
        let root = tempfile::tempdir().expect("making a temporary directory must succeed");
        let present = root.path().join("present");
        fs::create_dir_all(&present)
            .await
            .expect("making a folder must succeed");
        fs::write(present.join("a.jpg"), b"some bytes")
            .await
            .expect("writing a file must succeed");

        let walked = walk_mappings(&[
            Mapping {
                prefix: Some(parsed("albums")),
                local_root: root.path().join("never-created"),
                root_identity: None,
            },
            Mapping {
                prefix: None,
                local_root: present,
                root_identity: None,
            },
        ])
        .await
        .expect("a missing root is a verdict and not a failure");

        assert_eq!(
            walked.found.keys().cloned().collect::<Vec<_>>(),
            vec![parsed("a.jpg")],
            "the mapping whose root is there is walked as usual",
        );
        assert!(matches!(
            walked.roots[0].state,
            RootState::Unavailable(RootUnavailable::Missing),
        ));
        // Only where the platform can say what filesystem a folder stands on;
        // elsewhere a mapping records nothing and the root is simply available.
        #[cfg(unix)]
        assert!(
            matches!(walked.roots[1].state, RootState::Stamp(_)),
            "a root no scan has stamped yet is stamped with what this walk saw",
        );
        #[cfg(not(unix))]
        assert!(matches!(walked.roots[1].state, RootState::Available));
    }
}
