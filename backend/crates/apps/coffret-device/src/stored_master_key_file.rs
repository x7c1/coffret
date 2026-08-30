use std::fs;
use std::io::ErrorKind;

use coffret_format::{StoredMasterKey, UnlockedMasterKey};

use crate::error::{Error, Result};
use crate::library_dir::LibraryDir;
use crate::owner_only;

/// The file a device keeps its Master Key in, under the Passphrase.
///
/// `coffret-format` says what the bytes are (spec: KD-9) and deliberately does
/// no I/O; this says where they go and what they are protected with on the way
/// there. Nothing in the file is Passphrase-derived material that ever reaches
/// Storage, and nothing in it is read as key material before the whole file
/// authenticates — a Passphrase that is not the one it was written under yields
/// the format crate's own refusal and no bytes at all (spec: DK-5).
#[derive(Debug)]
pub struct StoredMasterKeyFile;

impl StoredMasterKeyFile {
    /// Writes the stored form as the Master Key of the Library in `dir`.
    pub fn write(dir: &LibraryDir, stored: &StoredMasterKey) -> Result<()> {
        owner_only::write_file(
            "writing the stored Master Key",
            &dir.master_key_file(),
            stored.as_bytes(),
        )
    }

    /// Reads the stored form back, without opening it.
    pub fn read(dir: &LibraryDir) -> Result<StoredMasterKey> {
        let path = dir.master_key_file();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(cause) if cause.kind() == ErrorKind::NotFound => {
                return Err(Error::NoSuchLibrary {
                    name: dir.name().to_owned(),
                    path: dir.path().to_path_buf(),
                })
            }
            Err(cause) => {
                return Err(Error::Local {
                    doing: "reading the stored Master Key",
                    path,
                    cause,
                })
            }
        };

        StoredMasterKey::from_bytes(bytes)
            .map_err(|cause| Error::MasterKeyNotUnlocked { path, cause })
    }

    /// Opens the Library's Master Key with the Passphrase that protects it.
    ///
    /// The key stays in memory for as long as the caller holds it and goes when
    /// the process does; one process is one unlock (spec: DK-9).
    pub fn unlock(dir: &LibraryDir, passphrase: &[u8]) -> Result<UnlockedMasterKey> {
        let stored = Self::read(dir)?;
        stored
            .unlock(passphrase)
            .map_err(|cause| Error::MasterKeyNotUnlocked {
                path: dir.master_key_file(),
                cause,
            })
    }
}
