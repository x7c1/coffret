use std::fs;

use coffret_format::RecoveryCode;
use coffret_model::{MasterKeyEpoch, Passphrase};

use super::{create_library, NewProvider};
use crate::device_settings::{DeviceSettings, ProviderSettings};
use crate::error::{Error, NameDefect};
use crate::library_dir::LibraryDir;
use crate::stored_master_key_file::StoredMasterKeyFile;
use crate::testing::{create_s3, passphrase, request, state_dir, PASSPHRASE, REGION};

/// A callback that fails the case if a Passphrase is ever asked for.
///
/// Every refusal made before a Library is staged is one that needs no key, and
/// asking for a Passphrase before making one is the defect these cases keep out.
fn unasked() -> crate::error::Result<Passphrase> {
    panic!("no Passphrase may be asked for before a refusal that needs none")
}

// DK-1: a device keeps one directory per Library, and everything it needs is
// in it. FM-18: the Library's place on Storage is its ID under the base the
// user chose.
#[tokio::test]
async fn a_created_library_holds_the_five_things_a_device_keeps() {
    let created = create_s3("five-things").await;
    let dir = LibraryDir::resolve("five-things").expect("the name is one component");

    assert_eq!(created.path, dir.path());
    for path in [dir.settings_file(), dir.master_key_file(), dir.index_file()] {
        assert!(path.is_file(), "{} must be a file", path.display());
    }
    assert!(dir.spool_dir().is_dir());
    // A Library that has never been synced has no grant to cache and no cache.
    assert!(!dir.token_cache_file().exists());
    // And nothing is left of the directory it was staged in.
    assert!(!dir.staging().path().exists());

    let settings = DeviceSettings::read(&dir).expect("the settings this build wrote must read");
    assert_eq!(settings, created.settings);
    let ProviderSettings::S3 { prefix, bucket, .. } = &settings.provider else {
        panic!("an S3 Library must be recorded as one: {settings:?}");
    };
    assert_eq!(bucket, "photos");
    assert_eq!(
        prefix,
        &format!("archive/coffret-{}/", settings.library_id.to_hex())
    );
}

// The Master Key, the catalog, and the settings are all owner-only from the
// moment they exist: the first is key material under a Passphrase, the second
// is plaintext that names Entry Paths, and the third can carry an OAuth client
// secret.
#[cfg(unix)]
#[tokio::test]
async fn what_a_device_keeps_is_readable_by_nobody_else() {
    use crate::testing::mode_of;

    create_s3("owner-only").await;
    let dir = LibraryDir::resolve("owner-only").expect("the name is one component");

    assert_eq!(mode_of(&dir.settings_file()), 0o600);
    assert_eq!(mode_of(&dir.master_key_file()), 0o600);
    assert_eq!(mode_of(&dir.index_file()), 0o600);
    assert_eq!(mode_of(dir.path()), 0o700);
    assert_eq!(mode_of(&dir.spool_dir()), 0o700);
}

// KD-11: the Recovery Code is the Master Key and its epoch, and a Library
// starts life at epoch 1. What the code carries has to be what the stored form
// opens to, or a device restored from it would open nothing.
#[tokio::test]
async fn the_recovery_code_carries_the_key_the_stored_form_opens_to() {
    let created = create_s3("recovery").await;
    let dir = LibraryDir::resolve("recovery").expect("the name is one component");

    let parsed = RecoveryCode::parse(created.recovery_code.as_str())
        .expect("a code this build wrote must parse back");
    let unlocked = StoredMasterKeyFile::unlock(&dir, &Passphrase::from_bytes(PASSPHRASE.to_vec()))
        .expect("the Passphrase must open the key");

    assert_eq!(
        parsed.master_key().as_bytes(),
        unlocked.master_key.as_bytes()
    );
    assert_eq!(parsed.epoch(), MasterKeyEpoch::FIRST);
    assert_eq!(unlocked.epoch, MasterKeyEpoch::FIRST);
    // The printed form and the bare one are one value, so what a person writes
    // down is what they can type back.
    assert_eq!(
        RecoveryCode::parse(&created.recovery_code.to_grouped_string())
            .expect("the printed form must parse too")
            .as_str(),
        created.recovery_code.as_str()
    );
}

// A second `init` over a Library would strand whatever the first one put on
// Storage, so it is refused — and refused without touching what is there.
#[tokio::test]
async fn a_second_library_of_one_name_is_refused_and_changes_nothing() {
    let created = create_s3("only-once").await;
    let dir = LibraryDir::resolve("only-once").expect("the name is one component");
    let key_before = fs::read(dir.master_key_file()).expect("the key file must be readable");

    let result = create_library(request("only-once"), passphrase, |_| ()).await;
    assert!(
        matches!(&result, Err(Error::LibraryExists { name, .. }) if name == "only-once"),
        "expected a second creation to be refused, got {result:?}"
    );

    assert_eq!(
        DeviceSettings::read(&dir).expect("the settings must still read"),
        created.settings
    );
    assert_eq!(
        fs::read(dir.master_key_file()).expect("the key file must still be readable"),
        key_before
    );
}

// Nothing in a directory an interrupted creation left ever reached Storage
// under a key anything kept, so it is discarded rather than resumed — and the
// creation that finds it succeeds.
#[tokio::test]
async fn a_directory_an_interrupted_creation_left_is_discarded() {
    state_dir();
    let dir = LibraryDir::resolve("leftover").expect("the name is one component");
    let staging = dir.staging();
    fs::create_dir_all(staging.path()).expect("the staging directory must be creatable");
    fs::write(staging.master_key_file(), b"half a Master Key")
        .expect("the leftover must be writable");

    let created = create_s3("leftover").await;

    assert!(!staging.path().exists());
    assert_eq!(
        DeviceSettings::read(&dir).expect("the settings must read"),
        created.settings
    );
    // The half-written key is gone rather than kept: what unlocks now is what
    // this creation wrote.
    StoredMasterKeyFile::unlock(&dir, &Passphrase::from_bytes(PASSPHRASE.to_vec()))
        .expect("the new Passphrase must open the key");
}

// The name becomes a directory name, so it is refused before any directory is
// made — including the staging one.
#[tokio::test]
async fn a_name_that_is_not_one_component_creates_nothing() {
    state_dir();
    let mut request = request("escape");
    request.name = "../escape".to_owned();

    let result = create_library(request, passphrase, |_| ()).await;
    assert!(
        matches!(
            &result,
            Err(Error::InvalidLibraryName {
                defect: NameDefect::Separator,
                ..
            })
        ),
        "expected a name with a separator to be refused, got {result:?}"
    );
    assert!(!state_dir().join("libraries").join("escape").exists());
}

// FM-18: on S3 a prefix exists by being written under, so a bucket that is not
// there is the one thing creating a Library has to ask Storage about. The answer
// is a value naming the bucket rather than a message about one, because that is
// what the explorer will read and what a case may assert on without matching on
// prose — and so is the cause, which is what tells an endpoint nothing is
// listening at apart from a bucket S3 answered about and does not hold. It is
// also a refusal that needs no key, so it is made before anybody is asked to
// choose a Passphrase.
#[tokio::test]
async fn a_bucket_that_does_not_answer_creates_nothing() {
    state_dir();
    let mut request = request("nowhere");
    request.provider = NewProvider::S3 {
        bucket: "absent-bucket".to_owned(),
        base_prefix: "archive/".to_owned(),
        // A port nothing is listening at, which is what a mistyped endpoint and
        // an implementation that is not running both look like.
        endpoint: Some("http://127.0.0.1:1".to_owned()),
        region: Some(REGION.to_owned()),
        path_style: true,
    };

    let result = create_library(request, unasked, |_| ()).await;
    assert!(
        matches!(
            &result,
            Err(Error::BucketUnreachable {
                bucket,
                cause: coffret_usecase::Error::Transport { .. },
            }) if bucket == "absent-bucket"
        ),
        "expected a bucket nothing answered for to be named and said to be unreachable, \
         got {result:?}"
    );
    assert!(!state_dir().join("libraries").join("nowhere").exists());
}

// The Passphrase arrives through a callback, so the caller that was asked is
// where a refusal to give one comes from — a terminal that reached the end of
// its input, or an explorer whose dialog was dismissed. It is not a failure this
// crate can produce, and what it leaves behind is what every other failure of a
// creation leaves: nothing. The directory the Library was being built in is
// removed on the way out, and the name it would have taken is still free.
#[tokio::test]
async fn a_passphrase_that_is_refused_creates_nothing() {
    state_dir();
    let dir = LibraryDir::resolve("unasked-for").expect("the name is one component");

    let result = create_library(
        request("unasked-for"),
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
