//! Which folders on this device stand for which part of the Library.
//!
//! A mapping is device state and is never uploaded (spec: CK-7), which is why
//! it lives in the catalog rather than in the settings file and why recording
//! one needs no Passphrase: the catalog is plaintext, and what a device calls
//! its own folders says nothing the Library keeps secret.

use coffret_sqlite_index::{RefusedIndex, SqliteIndex};
use coffret_usecase::{Index, IndexError};

use crate::error::{Error, Result};
use crate::library_dir::LibraryDir;
use crate::mapping_listing::MappingListing;

// The one place the marker a mapped root carries is written, adopted, or
// refused (spec: EP-13). It sits under this module because recording a mapping
// is the only thing that does any of it.
mod root_marker;

mod set_mapping;
pub use set_mapping::set_mapping;

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

#[cfg(test)]
mod tests;
