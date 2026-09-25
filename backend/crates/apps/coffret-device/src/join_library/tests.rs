use std::sync::Arc;

use coffret_format::RecoveryCode;
use coffret_model::Passphrase;
use zeroize::Zeroizing;

use super::run::{library_of_folder_name, library_of_prefix};
use super::{
    join_library, join_library_through, FoundOnStorage, JoinLibraryRequest, JoinedLibrary,
    JoinedProvider,
};
use crate::device_settings::{DeviceSettings, ProviderSettings};
use crate::error::{CreationStep, Error};
use crate::library_dir::LibraryDir;
use crate::reach::Reach;
use crate::stored_master_key_file::StoredMasterKeyFile;
use crate::testing::{
    consent, create_s3, every_link, state_dir, unopenable_catalog, DriveStub, CLIENT_ID, PASSPHRASE,
};

/// The Passphrase the joining device chooses, which is deliberately not the one
/// the Library was created under: the stored form is per device (spec: KD-9),
/// and nothing about a join asks what another device's is.
const OWN_PASSPHRASE: &[u8] = b"a second device, a second passphrase";

/// A callback that fails the case if a Passphrase is ever asked for.
///
/// Every refusal here is one that needs no key, and asking for a Passphrase
/// before making it is the defect these cases exist to keep out.
fn unasked_passphrase() -> crate::error::Result<Passphrase> {
    panic!("no Passphrase may be asked for before a refusal that needs none")
}

fn unasked_recovery_code() -> crate::error::Result<Zeroizing<String>> {
    panic!("no Recovery Code may be asked for before a refusal that needs none")
}

/// What joining `prefix` under `name` asks for.
fn request(name: &str, prefix: &str) -> JoinLibraryRequest {
    JoinLibraryRequest {
        name: name.to_owned(),
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
        request(name, prefix),
        || Ok(Zeroizing::new(code.to_grouped_string())),
        || Ok(Passphrase::from_bytes(OWN_PASSPHRASE.to_vec())),
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
    let unlocked =
        StoredMasterKeyFile::unlock(&dir, &Passphrase::from_bytes(OWN_PASSPHRASE.to_vec()))
            .expect("this device's own Passphrase must open its own stored form");
    assert_eq!(
        RecoveryCode::encode(unlocked.master_key, unlocked.epoch).as_str(),
        created.recovery_code.as_str()
    );
    assert!(
        StoredMasterKeyFile::unlock(&dir, &Passphrase::from_bytes(PASSPHRASE.to_vec())).is_err()
    );
}

// A device joining twice under one name would draw a second directory over the
// first, so it is refused — and refused before a Passphrase is asked for.
#[tokio::test]
async fn a_library_of_one_name_is_joined_once() {
    let created = create_s3("joined-once").await;
    let prefix = prefix_of(&created.settings);
    join("joined-twice", &created.recovery_code, &prefix).await;

    let result = join_library(
        request("joined-twice", &prefix),
        unasked_recovery_code,
        unasked_passphrase,
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
        request("no-passphrase", &prefix),
        || Ok(Zeroizing::new(created.recovery_code.to_grouped_string())),
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

#[tokio::test]
async fn a_recovery_code_that_is_not_given_joins_nothing() {
    let created = create_s3("recovery-code-asked").await;
    let prefix = prefix_of(&created.settings);
    let dir = LibraryDir::resolve("no-recovery-code").expect("the name is one component");

    let result = join_library(
        request("no-recovery-code", &prefix),
        || {
            Err(Error::RecoveryCodeNotGiven {
                cause: "standard input ended before a Recovery Code was given".into(),
            })
        },
        unasked_passphrase,
        |_| (),
    )
    .await;

    assert!(
        matches!(&result, Err(Error::RecoveryCodeNotGiven { .. })),
        "expected the caller's Recovery Code refusal to travel whole, got {result:?}"
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
        request("mistyped", &prefix),
        || Ok(Zeroizing::new(typed)),
        unasked_passphrase,
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
            request("elsewhere", asked),
            unasked_recovery_code,
            unasked_passphrase,
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

// A Library's own prefix is a prefix like any other until something has been
// written under it, so the join says what it found there rather than deciding
// what it means. Two joins reach this state and the flow cannot tell them
// apart: the Library these cases create has never been synced, and a Library ID
// with a character wrong is a perfectly well-formed prefix that holds nothing.
// Both are joined, and both say so — which is what lets a caller say one
// sentence that serves whichever of the two it was.
#[tokio::test]
async fn a_prefix_holding_nothing_of_a_library_is_joined_and_says_so() {
    let created = create_s3("never-synced").await;
    let its_own = prefix_of(&created.settings);

    let fresh = join("fresh-join", &created.recovery_code, &its_own).await;
    assert_eq!(
        fresh.found,
        FoundOnStorage::NothingYet,
        "a Library that has committed nothing holds nothing at its prefix",
    );

    // The same shape with a Library ID that is not this Library's: well-formed,
    // so nothing refuses it, and empty for the other of the two reasons.
    let mistyped = join(
        "mistyped-join",
        &created.recovery_code,
        "archive/coffret-0123456789abcdef/",
    )
    .await;
    assert_eq!(
        mistyped.found,
        FoundOnStorage::NothingYet,
        "a prefix that is not the Library's holds nothing either",
    );
}

// The contents question, which both providers are asked and which neither
// answers by refusing. On S3 the prefix is asked whether it holds the first
// link of the head chain and on Drive the app folder is, and the one answer is
// read the same way: a Library nobody has synced holds nothing wherever it
// lives, and the person joining it is owed that word rather than an empty
// `fetch` to make sense of. A Drive folder whose name is beyond doubt is
// exactly the case this is for — the name settles which Library it is and says
// nothing at all about whether anything has been committed into it.
#[test]
fn a_place_without_the_first_head_object_holds_nothing_of_the_library() {
    assert_eq!(FoundOnStorage::of(false), FoundOnStorage::NothingYet);
    assert_eq!(FoundOnStorage::of(true), FoundOnStorage::TheLibrary);
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
                .unwrap_or_else(|error| {
                    panic!("{prefix:?} names a Library: {}", every_link(&error))
                })
                .to_hex(),
            "0123456789abcdef"
        );
    }
}

/// The id the Drive a join case stands in for gave the Library's app folder.
const DRIVE_FOLDER_ID: &str = "stub-library-folder";

/// What joining the Drive folder [`DRIVE_FOLDER_ID`] under `name` asks for.
fn drive_request(name: &str) -> JoinLibraryRequest {
    JoinLibraryRequest {
        name: name.to_owned(),
        provider: JoinedProvider::Drive {
            folder_id: DRIVE_FOLDER_ID.to_owned(),
            client_id: CLIENT_ID.to_owned(),
            client_secret: None,
        },
    }
}

// The join's catalog is the same file a creation's is, and a disk that will not
// hold it stops the join at that step with nothing left behind: no staging
// directory, and nothing under the name that the next command could open.
#[tokio::test]
async fn a_catalog_that_will_not_open_leaves_no_joined_library_behind() {
    let created = create_s3("catalog-first").await;
    let prefix = prefix_of(&created.settings);
    let dir = LibraryDir::resolve("catalog-refused").expect("the name is one component");

    let result = join_library_through(
        &Reach::this_device().opening_index_with(unopenable_catalog),
        request("catalog-refused", &prefix),
        || Ok(Zeroizing::new(created.recovery_code.to_grouped_string())),
        || Ok(Passphrase::from_bytes(OWN_PASSPHRASE.to_vec())),
        |_| panic!("an S3 Library asks nobody for consent"),
    )
    .await;

    let Err(Error::LibraryNotJoined { step, cause, .. }) = &result else {
        panic!("expected the join to stop at the catalog, got {result:?}");
    };
    assert!(matches!(step, CreationStep::Index), "stopped at {step:?}");
    assert!(
        matches!(**cause, Error::Index { .. }),
        "the catalog's own refusal travels as the cause: {cause:?}",
    );
    assert!(!dir.staging().path().exists());
    assert!(!dir.path().exists());
}

// A Drive Library joined end to end below the terminal: the consent, the grant,
// the folder's name read back for the Library it names, and the folder asked
// whether anything of the Library is in it (spec: SA-1, FM-18). Every call goes
// through the gateway's own request building, and none of it reaches Google.
#[tokio::test]
async fn a_drive_library_is_joined_from_its_folder_s_name() {
    let created = create_s3("drive-origin").await;
    let library_id = created.settings.library_id;
    let drive = DriveStub::holding(DRIVE_FOLDER_ID, &library_id.app_folder_name());

    let joined = join_library_through(
        &Reach::this_device().reaching_drive_through(Arc::clone(&drive) as _),
        drive_request("joined-on-drive"),
        || Ok(Zeroizing::new(created.recovery_code.to_grouped_string())),
        || Ok(Passphrase::from_bytes(OWN_PASSPHRASE.to_vec())),
        consent,
    )
    .await
    .expect("a Recovery Code, a consent and the Library's folder are all a join needs");
    let dir = LibraryDir::resolve("joined-on-drive").expect("the name is one component");

    assert_eq!(
        joined.settings.library_id, library_id,
        "which Library this is comes out of the folder's name (spec: FM-18)",
    );
    assert!(matches!(joined.found, FoundOnStorage::TheLibrary));
    assert_eq!(
        joined.settings.provider,
        ProviderSettings::Drive {
            folder_id: DRIVE_FOLDER_ID.to_owned(),
            client_id: CLIENT_ID.to_owned(),
            client_secret: None,
        },
    );
    assert!(
        dir.token_cache_file().is_file(),
        "the grant is sealed into the joined Library's directory",
    );
    let unlocked =
        StoredMasterKeyFile::unlock(&dir, &Passphrase::from_bytes(OWN_PASSPHRASE.to_vec()))
            .expect("this device's own Passphrase opens its own stored form");
    assert_eq!(
        RecoveryCode::encode(unlocked.master_key, unlocked.epoch).as_str(),
        created.recovery_code.as_str(),
    );

    // The code traded for the grant and the grant for an access token, then
    // the folder's name and the listing of it — and nothing written: a join
    // creates nothing on Storage.
    assert_eq!(
        drive.asked(),
        ["token", "token", "name", "list"],
        "{:?}",
        drive.calls(),
    );
}

// A folder whose name is not a Library's is refused rather than recorded, and
// through Drive as through S3 the refusal leaves nothing on this device.
#[tokio::test]
async fn a_drive_folder_that_is_not_a_library_s_is_refused() {
    let created = create_s3("drive-misnamed-origin").await;
    let drive = DriveStub::holding(DRIVE_FOLDER_ID, "Holiday photos");
    let dir = LibraryDir::resolve("misnamed-on-drive").expect("the name is one component");

    let result = join_library_through(
        &Reach::this_device().reaching_drive_through(drive),
        drive_request("misnamed-on-drive"),
        || Ok(Zeroizing::new(created.recovery_code.to_grouped_string())),
        || Ok(Passphrase::from_bytes(OWN_PASSPHRASE.to_vec())),
        consent,
    )
    .await;

    assert!(
        matches!(
            &result,
            Err(Error::LibraryNotJoined {
                step: CreationStep::AppFolderName,
                ..
            })
        ),
        "expected the folder's name to be refused, got {result:?}",
    );
    assert!(!dir.staging().path().exists());
    assert!(!dir.path().exists());
}
