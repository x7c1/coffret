use std::fs;

use super::recovery_code;
use crate::error::Error;
use crate::library_dir::LibraryDir;
use crate::testing::{create_s3, PASSPHRASE};

// The code is written out again from the stored form rather than kept
// anywhere, so a person who lost the printout has not lost the code.
#[tokio::test]
async fn the_recovery_code_can_be_asked_for_again() {
    let created = create_s3("printed-again").await;

    let again = recovery_code("printed-again", PASSPHRASE).expect("the Passphrase must open it");
    assert_eq!(again.as_str(), created.recovery_code.as_str());
}

// DK-5: the stored form authenticates as a whole or not at all, so a Passphrase
// that is not the one it was written under yields no key rather than a
// different one — and nothing on the device is touched on the way to finding
// out.
#[tokio::test]
async fn a_wrong_passphrase_opens_nothing_and_changes_nothing() {
    create_s3("wrong-passphrase").await;
    let dir = LibraryDir::resolve("wrong-passphrase").expect("the name is one component");
    let before = fs::read(dir.master_key_file()).expect("the key file must be readable");

    for result in [
        crate::open_library("wrong-passphrase", b"not the Passphrase")
            .await
            .map(|_| ()),
        recovery_code("wrong-passphrase", b"not the Passphrase").map(|_| ()),
    ] {
        assert!(
            matches!(
                &result,
                Err(Error::MasterKeyNotUnlocked {
                    cause: coffret_format::Error::AuthenticationFailed,
                    ..
                })
            ),
            "expected the format layer's own refusal, got {result:?}"
        );
    }

    assert_eq!(
        fs::read(dir.master_key_file()).expect("the key file must still be readable"),
        before
    );
}
