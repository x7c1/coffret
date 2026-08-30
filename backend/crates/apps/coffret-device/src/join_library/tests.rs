use coffret_format::RecoveryCode;

use super::run::{library_of_folder_name, library_of_prefix};
use super::{join_library, JoinLibraryRequest, JoinedLibrary, JoinedProvider};
use crate::device_settings::{DeviceSettings, ProviderSettings};
use crate::error::Error;
use crate::library_dir::LibraryDir;
use crate::stored_master_key_file::StoredMasterKeyFile;
use crate::testing::{create_s3, state_dir, PASSPHRASE};

/// The Passphrase the joining device chooses, which is deliberately not the one
/// the Library was created under: the stored form is per device (spec: KD-9),
/// and nothing about a join asks what another device's is.
const OWN_PASSPHRASE: &[u8] = b"a second device, a second passphrase";

/// A callback that fails the case if a Passphrase is ever asked for.
///
/// Every refusal here is one that needs no key, and asking for a Passphrase
/// before making it is the defect these cases exist to keep out.
fn unasked() -> crate::error::Result<Vec<u8>> {
    panic!("no Passphrase may be asked for before a refusal that needs none")
}

/// What joining `prefix` under `name` asks for.
fn request(name: &str, code: &RecoveryCode, prefix: &str) -> JoinLibraryRequest {
    JoinLibraryRequest {
        name: name.to_owned(),
        recovery_code: code.to_grouped_string(),
        provider: JoinedProvider::S3 {
            bucket: "photos".to_owned(),
            prefix: prefix.to_owned(),
            endpoint: Some(crate::testing::stub_endpoint().to_owned()),
            region: Some("us-east-1".to_owned()),
            path_style: true,
        },
    }
}

/// The prefix a created Library recorded for itself.
fn prefix_of(settings: &DeviceSettings) -> String {
    let ProviderSettings::S3 { prefix, .. } = &settings.provider else {
        panic!("an S3 Library must be recorded as one");
    };
    prefix.clone()
}

/// Joins the Library `created` under `name`, and hands back what it recorded.
async fn join(name: &str, code: &RecoveryCode, prefix: &str) -> JoinedLibrary {
    join_library(
        request(name, code, prefix),
        || Ok(OWN_PASSPHRASE.to_vec()),
        |_| panic!("an S3 Library asks nobody for consent"),
    )
    .await
    .expect("a Recovery Code and a bucket that answers are all a join needs")
}

// The whole of what a second device gets from a Recovery Code: the same Library,
// the same Master Key at the epoch the code was written at, and a directory of
// its own with its own Passphrase on it.
#[tokio::test]
async fn a_joined_library_holds_the_same_master_key_under_this_device_s_passphrase() {
    let created = create_s3("first-device").await;
    let prefix = prefix_of(&created.settings);

    let joined = join("second-device", &created.recovery_code, &prefix).await;
    let dir = LibraryDir::resolve("second-device").expect("the name is one component");

    assert_eq!(joined.path, dir.path());
    for path in [dir.settings_file(), dir.master_key_file(), dir.index_file()] {
        assert!(path.is_file(), "{} must be a file", path.display());
    }
    assert!(dir.spool_dir().is_dir());
    assert!(!dir.staging().path().exists());

    // FM-18: which Library this is comes out of the prefix that was entered, not
    // out of anything drawn here.
    assert_eq!(joined.settings.library_id, created.settings.library_id);
    assert_eq!(prefix_of(&joined.settings), prefix);

    // KD-9, KD-11: the key is the code's, at the code's epoch, under this
    // device's own Passphrase — and not under the one the Library was created
    // with.
    let unlocked = StoredMasterKeyFile::unlock(&dir, OWN_PASSPHRASE)
        .expect("this device's own Passphrase must open its own stored form");
    assert_eq!(
        RecoveryCode::encode(&unlocked.master_key, unlocked.epoch).as_str(),
        created.recovery_code.as_str()
    );
    assert!(StoredMasterKeyFile::unlock(&dir, PASSPHRASE).is_err());
}

// A device joining twice under one name would draw a second directory over the
// first, so it is refused — and refused before a Passphrase is asked for.
#[tokio::test]
async fn a_library_of_one_name_is_joined_once() {
    let created = create_s3("joined-once").await;
    let prefix = prefix_of(&created.settings);
    join("joined-twice", &created.recovery_code, &prefix).await;

    let result = join_library(
        request("joined-twice", &created.recovery_code, &prefix),
        unasked,
        |_| (),
    )
    .await;
    assert!(
        matches!(&result, Err(Error::LibraryExists { name, .. }) if name == "joined-twice"),
        "expected a second join of one name to be refused, got {result:?}"
    );
}

// The Passphrase this device chooses arrives through a callback, so a refusal to
// give one is the caller's and travels whole. By then every refusal that needs
// no key has passed and a directory is open — and it is removed on the way out,
// which leaves the name free and the Library on Storage untouched.
#[tokio::test]
async fn a_passphrase_that_is_refused_joins_nothing() {
    let created = create_s3("passphrase-asked").await;
    let prefix = prefix_of(&created.settings);
    let dir = LibraryDir::resolve("no-passphrase").expect("the name is one component");

    let result = join_library(
        request("no-passphrase", &created.recovery_code, &prefix),
        || {
            Err(Error::PassphraseNotGiven {
                cause: "standard input ended before a Passphrase was given".into(),
            })
        },
        |_| (),
    )
    .await;

    assert!(
        matches!(&result, Err(Error::PassphraseNotGiven { .. })),
        "expected the caller's own refusal to travel whole, got {result:?}"
    );
    assert!(!dir.staging().path().exists());
    assert!(!dir.path().exists());
}

// KD-11: a code with a mistyped character yields no Master Key rather than a
// different one, and the check it failed travels as the format crate named it.
#[tokio::test]
async fn a_code_that_is_not_one_is_refused_as_the_format_layer_refused_it() {
    let created = create_s3("code-checked").await;
    let prefix = prefix_of(&created.settings);

    // One character changed for another the alphabet holds, which is exactly the
    // hand-copying mistake the checksum is there to catch.
    let mut typed = created.recovery_code.as_str().to_owned();
    let last = typed.pop().expect("a code is never empty");
    typed.push(if last == 'q' { 'p' } else { 'q' });
    let result = join_library(
        JoinLibraryRequest {
            recovery_code: typed,
            ..request("mistyped", &created.recovery_code, &prefix)
        },
        unasked,
        |_| (),
    )
    .await;

    assert!(
        matches!(
            &result,
            Err(Error::MalformedRecoveryCode {
                cause: coffret_format::Error::RecoveryCodeChecksumFailed
            })
        ),
        "expected the format layer's own refusal, got {result:?}"
    );
    assert!(!state_dir().join("libraries").join("mistyped").exists());
}

// FM-18: a Library's keys start at its own folder's name, so a prefix that does
// not end in one names somewhere else entirely — and recording it would point
// this device at a place nothing else is configured against.
#[tokio::test]
async fn a_prefix_that_is_not_a_library_s_own_is_refused() {
    let created = create_s3("prefix-checked").await;
    let prefix = prefix_of(&created.settings);

    for (asked, what) in [
        // The base a Library was created under rather than the Library's own
        // prefix: it holds every Library kept at that location.
        ("archive/", "a base prefix"),
        // The right folder without the separator that puts the keys inside it.
        (
            prefix.trim_end_matches('/'),
            "a prefix with no trailing slash",
        ),
        // The right shape carrying something that is not a Library ID.
        ("archive/coffret-not-hex-at-all/", "a malformed Library ID"),
    ] {
        let result = join_library(
            request("elsewhere", &created.recovery_code, asked),
            unasked,
            |_| (),
        )
        .await;
        assert!(
            matches!(
                &result,
                Err(Error::NotALibraryFolder { location, .. }) if location == asked
            ),
            "expected {what} to be refused, got {result:?}"
        );
    }
    assert!(!state_dir().join("libraries").join("elsewhere").exists());
}

// The same rule the prefix is held to, on the one thing Drive has instead of a
// prefix: the folder's name is what says which Library it holds, so a folder
// called anything else is refused rather than recorded under an ID invented
// here (spec: FM-18).
#[test]
fn only_a_folder_named_after_a_library_names_one() {
    let named = library_of_folder_name("coffret-0123456789abcdef")
        .expect("a folder named after a Library names it");
    assert_eq!(named.to_hex(), "0123456789abcdef");

    for name in [
        "photos",
        "coffret",
        // The prefix and too few characters after it.
        "coffret-0123456789abcde",
        // Uppercase is a second spelling of one ID, which would name one folder
        // twice.
        "coffret-0123456789ABCDEF",
    ] {
        assert!(
            matches!(
                library_of_folder_name(name),
                Err(Error::NotALibraryFolder { .. })
            ),
            "{name:?} must not name a Library"
        );
    }
}

// A prefix is that same name as a key prefix, under whatever base the person
// chose — including no base at all.
#[test]
fn a_prefix_names_the_library_its_last_component_does() {
    for prefix in [
        "coffret-0123456789abcdef/",
        "archive/coffret-0123456789abcdef/",
        "photos/2026/coffret-0123456789abcdef/",
    ] {
        assert_eq!(
            library_of_prefix(prefix)
                .unwrap_or_else(|error| panic!("{prefix:?} names a Library: {error}"))
                .to_hex(),
            "0123456789abcdef"
        );
    }
}
