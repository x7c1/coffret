use std::fs;

use coffret_model::Passphrase;

use super::recovery_code;
use crate::error::Error;
use crate::library_dir::LibraryDir;
use crate::testing::{create_s3, passphrase};

// The code is written out again from the stored form rather than kept
// anywhere, so a person who lost the printout has not lost the code.
#[tokio::test]
async fn the_recovery_code_can_be_asked_for_again() {
    let created = create_s3("printed-again").await;

    let again = recovery_code("printed-again", passphrase).expect("the Passphrase must open it");
    assert_eq!(again.as_str(), created.recovery_code.as_str());
}

// A mistyped Library name is the refusal a person meets most often, and it needs
// no key: asking for a Passphrase first would have them type one — or a script
// spend one — only to be told there is no such Library. Every other command that
// opens a Library answers this from the settings file before asking; this one has
// only the stored form to read, so it reads it first.
#[tokio::test]
async fn a_library_that_is_not_here_is_refused_before_a_passphrase_is_read() {
    create_s3("asked-for-by-the-wrong-name").await;
    let unasked = || panic!("no Passphrase may be asked for before a refusal that needs none");

    let result = recovery_code("asked-for-by-the-wrong-nmae", unasked);
    assert!(
        matches!(&result, Err(Error::NoSuchLibrary { name, .. }) if name == "asked-for-by-the-wrong-nmae"),
        "expected the name to be refused, got {result:?}"
    );
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
    let wrong = || Ok(Passphrase::from_bytes(b"not the Passphrase".to_vec()));

    for result in [
        crate::open_library("wrong-passphrase", wrong)
            .await
            .map(|_| ()),
        recovery_code("wrong-passphrase", wrong).map(|_| ()),
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
