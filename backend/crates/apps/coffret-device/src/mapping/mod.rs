//! Which folders on this device stand for which part of the Library.
//!
//! A mapping is device state and is never uploaded (spec: CK-7), which is why
//! it lives in the catalog rather than in the settings file and why recording
//! one needs no Passphrase: the catalog is plaintext, and what a device calls
//! its own folders says nothing the Library keeps secret.

use std::fs;
use std::path::{Path, PathBuf};

use coffret_model::EntryPath;
use coffret_sqlite_index::{RefusedIndex, SqliteIndex};
use coffret_usecase::device_state::Mapping;
use coffret_usecase::{Index, IndexError};

use crate::error::{Error, Result};
use crate::library_dir::LibraryDir;
use crate::mapping_listing::MappingListing;

/// Records that `local_root` on this device holds `prefix` of the Library, and
/// reports the mapping it replaced.
///
/// `prefix` is one top-level component of the Library, or `None` for the
/// Library root itself (spec: EP-9). Recording a prefix that is already mapped
/// replaces the root it stood for, which is what makes this the one call for
/// both mapping a folder and moving one — and why what it replaced comes back
/// rather than being dropped. Moving a mapping takes everything under the old
/// root out of the Library's reach on this device, so a caller that cannot say
/// what was there cannot tell a person what just happened.
///
/// `local_root` has to be a directory that is there now. A mapped root that has
/// gone missing is an ordinary state a scan reports when it looks
/// (spec: EP-12); a root that was never there is a typo, and recording it would
/// turn every later scan into that report.
pub async fn set_mapping(
    name: &str,
    prefix: Option<&str>,
    local_root: &Path,
) -> Result<Option<Mapping>> {
    let dir = open(name)?;
    let mapping = Mapping {
        prefix: prefix.map(entry_path).transpose()?,
        local_root: existing_directory(local_root)?,
        // Nothing yet: the next scan stamps whichever filesystem it finds the
        // root standing on (spec: EP-12).
        root_identity: None,
    };

    // Read before the write rather than after: one prefix holds one mapping, so
    // afterwards there is nothing left to have replaced.
    let index = index(&dir)?;
    let replaced = index
        .mappings()
        .await?
        .into_iter()
        .find(|recorded| recorded.prefix == mapping.prefix);

    index.set_mapping(mapping).await?;
    Ok(replaced)
}

/// What this device has mapped, the Library root first.
///
/// Root first because that is the order the mappings are read in: the root
/// mapping stands for everything the top-level ones do not (spec: EP-9), so it
/// is the one to see before the exceptions to it.
///
/// A Library whose Index this build cannot open is not a dead end for this
/// call: the two columns a mapping needs stay readable in a file of any
/// layout, so a refusal here comes back as
/// [`MappingListing::FromRefusedFile`] instead of failing outright — the
/// mappings read straight out of the file, carried with the refusal that says
/// why the catalog itself would not open.
pub async fn mappings(name: &str) -> Result<MappingListing> {
    let dir = open(name)?;
    match index(&dir) {
        Ok(index) => {
            let mut recorded = index.mappings().await?;
            recorded.sort_by(|left, right| left.prefix.cmp(&right.prefix));
            Ok(MappingListing::Recorded(recorded))
        }
        // A refusal is not a dead end here: the mappings are the one piece of
        // device state a refused file still gives up, so this reads them
        // straight from it instead of stopping at the refusal.
        Err(Error::Index {
            cause: refusal @ IndexError::UnsupportedSchema { .. },
        }) => {
            let mut read = RefusedIndex::open(dir.index_file())?.mappings()?;
            read.sort_by(|left, right| left.prefix.cmp(&right.prefix));
            Ok(MappingListing::FromRefusedFile {
                mappings: read,
                refusal,
            })
        }
        Err(other) => Err(other),
    }
}

/// The directory of a Library that is really on this device.
///
/// Asked before the catalog is opened, because opening a catalog creates one:
/// a name with a typo in it would otherwise leave an empty Library behind
/// instead of being refused.
fn open(name: &str) -> Result<LibraryDir> {
    let dir = LibraryDir::resolve(name)?;
    if !dir.is_present() {
        return Err(Error::NoSuchLibrary {
            name: dir.name().to_owned(),
            path: dir.path().to_path_buf(),
        });
    }
    Ok(dir)
}

/// The Library's catalog.
fn index(dir: &LibraryDir) -> Result<SqliteIndex> {
    SqliteIndex::open(dir.index_file()).map_err(Error::from)
}

/// The prefix as the Library spells it, or a refusal.
///
/// Two questions in the order they are owed. Whether the text is an Entry Path
/// at all is the type's — it becomes NFC on the way in and is held to the shape
/// every Entry Path is in (spec: EP-1, EP-2) — and whether it is one a mapping
/// can stand for is this crate's: a mapping is keyed by exactly one top-level
/// component, so a path of more than one names a subtree no mapping represents
/// (spec: EP-9).
///
/// Nothing else is asked. A backslash and every other character an Entry Path
/// may carry is carried here too: this names a folder inside the Library, not a
/// directory on this device, and the shape of the one says nothing about the
/// other.
fn entry_path(prefix: &str) -> Result<EntryPath> {
    let malformed = |cause| Error::MalformedMappingPrefix {
        prefix: prefix.to_owned(),
        cause,
    };
    let path = EntryPath::parse(prefix).map_err(|cause| malformed(Some(cause)))?;
    if path.top_level() != path.as_str() {
        return Err(malformed(None));
    }
    Ok(path)
}

/// The root as one absolute path with no symlinks left in it, or a refusal.
///
/// Canonicalised because a mapping outlives the working directory the command
/// was run from, and a relative root would mean a different folder the next
/// time anything read it.
fn existing_directory(local_root: &Path) -> Result<PathBuf> {
    let canonical = fs::canonicalize(local_root).map_err(|cause| Error::NoSuchLocalRoot {
        path: local_root.to_path_buf(),
        cause: Some(cause),
    })?;
    if !canonical.is_dir() {
        return Err(Error::NoSuchLocalRoot {
            path: canonical,
            cause: None,
        });
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests;
