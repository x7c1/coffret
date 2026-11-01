use std::cell::Cell;
use std::fs;
use std::io::{self, Read};

use coffret_format::{Error as FormatError, StoredMasterKey};

use super::{read_bounded, StoredMasterKeyFile};
use crate::error::Error;
use crate::library_dir::LibraryDir;
use crate::testing::{create_s3, passphrase};

/// An endless source of bytes that counts how many were taken from it.
struct Counting<'a> {
    taken: &'a Cell<usize>,
}

impl Read for Counting<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        buf.fill(0);
        self.taken.set(self.taken.get() + buf.len());
        Ok(buf.len())
    }
}

// KD-9: no valid stored form is longer than `MAX_LEN`, so a source longer than
// that is refused after reading one byte past it, however long it goes on.
#[test]
fn an_oversized_source_is_refused_after_reading_at_most_the_bound() {
    let taken = Cell::new(0);
    let read = read_bounded(Counting { taken: &taken }).expect("the source never fails");
    assert_eq!(read, None);
    assert_eq!(taken.get(), StoredMasterKey::MAX_LEN + 1);
}

#[test]
fn a_source_of_the_longest_valid_length_is_read_whole() {
    let bytes = vec![7; StoredMasterKey::MAX_LEN];
    assert_eq!(
        read_bounded(bytes.as_slice()).expect("a slice never fails"),
        Some(bytes)
    );
}

// KD-9: an oversized file is the format's own length refusal, met before any
// Passphrase is asked for.
#[tokio::test]
async fn an_oversized_file_is_refused_before_the_passphrase_is_asked_for() {
    create_s3("oversized-master-key").await;
    let dir = LibraryDir::named_here("oversized-master-key");
    fs::write(dir.master_key_file(), vec![0; 1 << 20]).expect("the file must be writable");

    let result = StoredMasterKeyFile::unlock_asking(&dir, || {
        panic!("a file that is no stored form must not cost a Passphrase")
    });
    assert!(
        matches!(
            result,
            Err(Error::MasterKeyNotUnlocked {
                cause: FormatError::StoredMasterKeyLengthMismatch,
                ..
            })
        ),
        "expected the length refusal, got {result:?}"
    );
}

// KD-9: a file recording a cost above the ceiling is refused as such — not as
// a wrong Passphrase — before any derivation, and before a Passphrase is even
// asked for where the caller asks for one only once the file is read.
#[tokio::test]
async fn a_file_asking_for_too_much_work_is_refused_without_checking_the_passphrase() {
    create_s3("costly-master-key").await;
    let dir = LibraryDir::named_here("costly-master-key");
    let mut bytes = fs::read(dir.master_key_file()).expect("the file must be readable");
    // The iterations field of the fixed part (spec: KD-9), at its largest value.
    bytes[12..16].copy_from_slice(&u32::MAX.to_be_bytes());
    fs::write(dir.master_key_file(), bytes).expect("the file must be writable");

    let asked = StoredMasterKeyFile::unlock_asking(&dir, || {
        panic!("a form refused for its cost must not cost a Passphrase")
    });
    assert!(
        matches!(
            asked,
            Err(Error::MasterKeyCostAboveCeiling {
                cause: FormatError::Argon2CostAboveCeiling {
                    parameter: "iterations",
                    value: u32::MAX,
                    ceiling: 8,
                },
                ..
            })
        ),
        "expected the cost refusal, got {asked:?}"
    );

    let given = StoredMasterKeyFile::unlock(&dir, &passphrase().expect("a fixed Passphrase"));
    let refused = given.expect_err("the form must be refused");
    assert!(matches!(refused, Error::MasterKeyCostAboveCeiling { .. }));
    let line = refused.to_string();
    assert!(
        line.contains("more work than coffret allows") && line.contains("was not checked"),
        "the line must not read as a wrong Passphrase: {line}"
    );
}
