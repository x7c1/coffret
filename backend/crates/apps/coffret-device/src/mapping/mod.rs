//! Which folders on this device stand for which part of the Library.
//!
//! A mapping is device state and is never uploaded (spec: CK-7), which is why
//! it lives in the catalog rather than in the settings file and why recording
//! one needs no Passphrase: the catalog is plaintext, and what a device calls
//! its own folders says nothing the Library keeps secret.

use std::fs;
use std::path::{Path, PathBuf};

use coffret_model::EntryPath;
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::device_state::Mapping;
use coffret_usecase::Index;

use crate::error::{Error, Result};
use crate::library_dir::LibraryDir;
use crate::name_defect::defect_in;

/// Records that `local_root` on this device holds `prefix` of the Library.
///
/// `prefix` is one top-level component of the Library, or `None` for the
/// Library root itself (spec: EP-9). Recording a prefix that is already mapped
/// replaces the root it stood for, which is what makes this the one call for
/// both mapping a folder and moving one.
///
/// `local_root` has to be a directory that is there now. A mapped root that has
/// gone missing is an ordinary state a scan reports when it looks
/// (spec: EP-12); a root that was never there is a typo, and recording it would
/// turn every later scan into that report.
pub async fn set_mapping(name: &str, prefix: Option<&str>, local_root: &Path) -> Result<()> {
    let dir = open(name)?;
    let mapping = Mapping {
        prefix: prefix.map(entry_path).transpose()?,
        local_root: existing_directory(local_root)?,
        // Nothing yet: the next scan stamps whichever filesystem it finds the
        // root standing on (spec: EP-12).
        root_identity: None,
    };

    index(&dir)?.set_mapping(mapping).await.map_err(Error::from)
}

/// What this device has mapped, the Library root first.
///
/// Root first because that is the order the mappings are read in: the root
/// mapping stands for everything the top-level ones do not (spec: EP-9), so it
/// is the one to see before the exceptions to it.
pub async fn mappings(name: &str) -> Result<Vec<Mapping>> {
    let dir = open(name)?;
    let mut recorded = index(&dir)?.mappings().await?;
    recorded.sort_by(|left, right| left.prefix.cmp(&right.prefix));
    Ok(recorded)
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
/// Text from outside the Library becomes NFC on the way in (spec: EP-1), and a
/// prefix that is not one top-level component names a subtree no mapping can
/// stand for.
fn entry_path(prefix: &str) -> Result<EntryPath> {
    match defect_in(prefix) {
        Some(defect) => Err(Error::MalformedMappingPrefix {
            prefix: prefix.to_owned(),
            defect,
        }),
        None => Ok(EntryPath::nfc(prefix)),
    }
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
