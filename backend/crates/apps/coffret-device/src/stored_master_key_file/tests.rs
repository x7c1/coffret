use std::cell::Cell;
use std::fs;
use std::io::{self, Read};

use coffret_format::{Error as FormatError, StoredMasterKey};

use super::{not_opened, read_bounded, StoredMasterKeyFile};
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
// Passphrase is asked for, and told as a file coffret cannot read rather than
// as a wrong Passphrase.
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
            Err(Error::MasterKeyFileMalformed {
                cause: FormatError::StoredMasterKeyLengthMismatch,
                ..
            })
        ),
        "expected the length refusal, got {result:?}"
    );
}

/// The Library `name`'s Master Key file, edited by `edit`.
async fn with_edited_file(name: &str, edit: impl FnOnce(&mut Vec<u8>)) -> LibraryDir {
    create_s3(name).await;
    let dir = LibraryDir::named_here(name);
    let mut bytes = fs::read(dir.master_key_file()).expect("the file must be readable");
    edit(&mut bytes);
    fs::write(dir.master_key_file(), bytes).expect("the file must be writable");
    dir
}

// KD-9: a file that is no stored form — the wrong length, an unknown magic or
// version, a non-zero reserved byte, a cost Argon2id itself refuses — is
// refused before any Passphrase is asked for, and never as a Passphrase that
// does not open it.
#[tokio::test]
async fn a_malformed_file_is_refused_as_such_without_checking_the_passphrase() {
    type Edit = fn(&mut Vec<u8>);
    type Expected = fn(&FormatError) -> bool;
    let cases: [(&str, Edit, Expected); 5] = [
        (
            "short-master-key",
            |bytes| bytes.truncate(40),
            |cause| matches!(cause, FormatError::StoredMasterKeyLengthMismatch),
        ),
        (
            "magic-master-key",
            |bytes| bytes[0] = b'X',
            |cause| matches!(cause, FormatError::UnknownStoredMasterKeyMagic { .. }),
        ),
        (
            "version-master-key",
            |bytes| bytes[5] = 0x02,
            |cause| {
                matches!(
                    cause,
                    FormatError::UnsupportedStoredMasterKeyVersion { actual: 0x02 }
                )
            },
        ),
        (
            "reserved-master-key",
            |bytes| bytes[6] = 0x01,
            |cause| matches!(cause, FormatError::ReservedNotZero),
        ),
        (
            "memoryless-master-key",
            // The memory field of the fixed part (spec: KD-9), below the least
            // Argon2id accepts.
            |bytes| bytes[8..12].copy_from_slice(&0u32.to_be_bytes()),
            |cause| matches!(cause, FormatError::InvalidArgon2Params { .. }),
        ),
    ];
    for (name, edit, expected) in cases {
        let dir = with_edited_file(name, edit).await;

        let asked = StoredMasterKeyFile::unlock_asking(&dir, || {
            panic!("a file that is no stored form must not cost a Passphrase")
        });
        assert!(
            matches!(&asked, Err(Error::MasterKeyFileMalformed { cause, .. }) if expected(cause)),
            "{name}: expected the malformed-file refusal, got {asked:?}"
        );

        let given = StoredMasterKeyFile::unlock(&dir, &passphrase().expect("a fixed Passphrase"));
        let refused = given.expect_err("the file must be refused");
        assert!(
            matches!(refused, Error::MasterKeyFileMalformed { .. }),
            "{name}: {refused:?}"
        );
        let line = refused.to_string();
        assert!(
            line.contains("not one coffret can read")
                && line.contains("was not checked")
                && !line.contains("may not be the one"),
            "{name}: the line must not read as a wrong Passphrase: {line}"
        );
    }
}

// DK-5: a wrong Passphrase on a well-formed file is still the failed
// authentication, and only that is told as a Passphrase that may not be the
// one the file was written under.
#[tokio::test]
async fn a_wrong_passphrase_on_a_well_formed_file_is_not_unlocked() {
    create_s3("wrong-for-the-file").await;
    let dir = LibraryDir::named_here("wrong-for-the-file");

    let refused = StoredMasterKeyFile::unlock(
        &dir,
        &coffret_model::Passphrase::from_bytes(b"not the Passphrase".to_vec()),
    )
    .expect_err("another Passphrase opens nothing");
    assert!(
        matches!(
            refused,
            Error::MasterKeyNotUnlocked {
                cause: FormatError::AuthenticationFailed,
                ..
            }
        ),
        "{refused:?}"
    );
    assert!(refused.to_string().contains("may not be the one"));
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

// KD-9: a stored form read whole that does not open for a reason other than a
// failed authentication — here an authentic form holding no epoch — is no
// answer about the Passphrase, and is refused naming the Library's key file;
// only a failed authentication is told as a Passphrase that may be wrong.
#[tokio::test]
async fn a_stored_form_that_opens_to_no_master_key_is_refused_naming_the_file() {
    create_s3("not-derived").await;
    let dir = LibraryDir::named_here("not-derived");

    let refused = not_opened(
        &dir,
        FormatError::StoredMasterKeyEpochOutOfRange { epoch: 0 },
    );
    match &refused {
        Error::MasterKeyNotDerived {
            path,
            cause: FormatError::StoredMasterKeyEpochOutOfRange { epoch: 0 },
        } => assert_eq!(path, &dir.master_key_file()),
        other => panic!("expected the form to be refused as not derived, got {other:?}"),
    }
    let line = refused.to_string();
    assert!(
        line.contains("other than the Passphrase") && !line.contains("may not be the one"),
        "the line must not read as a wrong Passphrase: {line}"
    );

    assert!(matches!(
        not_opened(&dir, FormatError::AuthenticationFailed),
        Error::MasterKeyNotUnlocked {
            cause: FormatError::AuthenticationFailed,
            ..
        }
    ));
}

// KD-9: a Library found on this device whose stored Master Key file is not
// there is refused naming the file, before any Passphrase is asked for, and
// never as no Library at all nor as a wrong Passphrase.
#[tokio::test]
async fn a_missing_file_is_refused_naming_it_before_the_passphrase_is_asked_for() {
    create_s3("missing-master-key").await;
    let dir = LibraryDir::named_here("missing-master-key");
    fs::remove_file(dir.master_key_file()).expect("the file must be removable");

    let asked = StoredMasterKeyFile::unlock_asking(&dir, || {
        panic!("a missing file must not cost a Passphrase")
    });
    let given = StoredMasterKeyFile::unlock(&dir, &passphrase().expect("a fixed Passphrase"));
    for refused in [asked, given] {
        match refused {
            Err(Error::MasterKeyFileMissing { path }) => assert_eq!(path, dir.master_key_file()),
            other => panic!("expected the file to be refused as missing, got {other:?}"),
        }
    }
    let line = Error::MasterKeyFileMissing {
        path: dir.master_key_file(),
    }
    .to_string();
    assert!(
        line.contains("is missing") && !line.contains("may not be the one"),
        "the line must not read as a wrong Passphrase: {line}"
    );
}
