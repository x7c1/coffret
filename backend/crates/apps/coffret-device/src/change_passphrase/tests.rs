use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use coffret_model::Passphrase;

use super::change_passphrase;
use crate::error::Error;
use crate::library_dir::LibraryDir;
use crate::recovery_code;
use crate::testing::{create_s3, passphrase};

/// The Passphrase each case changes to.
const NEW_PASSPHRASE: &[u8] = b"a passphrase chosen later";

fn new_passphrase() -> crate::error::Result<Passphrase> {
    Ok(Passphrase::from_bytes(NEW_PASSPHRASE.to_vec()))
}

/// Every file under `dir` but the stored Master Key, by path, with its bytes.
fn everything_else(dir: &LibraryDir) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(path: &Path, skip: &Path, into: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(path).expect("the directory must be listable") {
            let path = entry.expect("the entry must be readable").path();
            if path.is_dir() {
                walk(&path, skip, into);
            } else if path != skip {
                let bytes = fs::read(&path).expect("the file must be readable");
                into.insert(path, bytes);
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(dir.path(), &dir.master_key_file(), &mut files);
    files
}

// DK-6: the change re-protects the stored form and nothing else. The Master Key
// and its epoch are the ones the old Passphrase opened — which is what the
// Recovery Code written out under each spells — and every other file the
// Library keeps on this device, its settings among them, is as it was.
#[tokio::test]
async fn the_stored_form_opens_under_the_new_passphrase_to_the_same_key() {
    let created = create_s3("changed").await;
    let dir = LibraryDir::named_here("changed");
    let before = everything_else(&dir);
    assert!(
        before.contains_key(&dir.settings_file()),
        "the settings are among what is compared"
    );

    let changed = change_passphrase(
        &crate::LibraryRef::named("changed"),
        passphrase,
        new_passphrase,
    )
    .expect("the current Passphrase opens the stored form");
    assert_eq!(changed.name.as_deref(), Some("changed"));
    assert_eq!(changed.library_id, created.settings.library_id);

    let (reopened, _) = recovery_code(&crate::LibraryRef::named("changed"), new_passphrase)
        .expect("the new Passphrase opens it");
    assert_eq!(reopened.as_str(), created.recovery_code.as_str());
    let refused = recovery_code(&crate::LibraryRef::named("changed"), passphrase);
    assert!(
        matches!(
            &refused,
            Err(Error::MasterKeyNotUnlocked {
                cause: coffret_format::Error::AuthenticationFailed,
                ..
            })
        ),
        "the old Passphrase must open nothing, got {refused:?}"
    );
    assert_eq!(everything_else(&dir), before);
}

// DK-5: a wrong current Passphrase is refused by the stored form itself, before
// a new one is asked for, and the file is left byte for byte as it was.
#[tokio::test]
async fn a_wrong_current_passphrase_changes_nothing() {
    create_s3("not-changed").await;
    let dir = LibraryDir::named_here("not-changed");
    let before = fs::read(dir.master_key_file()).expect("the key file must be readable");
    let wrong = || Ok(Passphrase::from_bytes(b"not the Passphrase".to_vec()));
    let unasked = || panic!("no new Passphrase may be asked for under a wrong current one");

    let result = change_passphrase(&crate::LibraryRef::named("not-changed"), wrong, unasked);
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
    assert_eq!(
        fs::read(dir.master_key_file()).expect("the key file must still be readable"),
        before
    );
}

// A mistyped name is refused from the filesystem alone, so it costs nobody a
// Passphrase of either kind.
#[tokio::test]
async fn a_library_that_is_not_here_is_refused_before_a_passphrase_is_read() {
    create_s3("changed-by-name").await;
    let unasked = || panic!("no Passphrase may be asked for before a refusal that needs none");

    let result = change_passphrase(
        &crate::LibraryRef::named("changed-by-nmae"),
        unasked,
        unasked,
    );
    assert!(
        matches!(&result, Err(Error::NoSuchLibrary { library, .. }) if *library == crate::LibraryRef::named("changed-by-nmae")),
        "expected the name to be refused, got {result:?}"
    );
}
