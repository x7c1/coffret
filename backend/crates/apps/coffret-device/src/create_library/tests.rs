use std::fs;
use std::ops::Range;
use std::sync::Arc;

use async_trait::async_trait;
use coffret_format::RecoveryCode;
use coffret_model::{ControlObjectName, Generation, MasterKeyEpoch, ObjectRef, Passphrase};
use coffret_usecase::catch_up::{catch_up_catalog, CatchUpRequest};
use coffret_usecase::{
    ByteStream, CommitSlot, InMemoryIndex, InMemoryMintingStore, InMemoryStore, Index, LibraryKeys,
    ObjectPage, ObjectStore, PageToken, UploadedObject,
};

use super::{create_library, create_library_through, NewProvider};
use crate::device_settings::{DeviceSettings, ProviderSettings};
use crate::error::{CreationStep, Error};
use crate::library_dir::{isolated, LibraryDir};
use crate::reach::Reach;
use crate::stored_master_key_file::StoredMasterKeyFile;
use crate::testing::{
    consent, create_s3, local_fs, passphrase, request, state_dir, unopenable_catalog, DriveStub,
    CLIENT_ID, CREATED_FOLDER_ID, PASSPHRASE, REGION,
};
use crate::{LibraryNameDefect, LibraryRef};

/// A callback that fails the case if a Passphrase is ever asked for.
///
/// Every refusal made before a Library is staged is one that needs no key, and
/// asking for a Passphrase before making one is the defect these cases keep out.
fn unasked() -> crate::error::Result<Passphrase> {
    panic!("no Passphrase may be asked for before a refusal that needs none")
}

// DK-1: a device keeps one directory per Library, and everything it needs is
// in it. FM-18: the Library's place on Storage is its ID under the base the
// user chose. FM-15: its name is its own, in its first head. On the device the
// directory is named after the Library's ID, and the settings keep a copy of
// the name.
#[tokio::test]
async fn a_created_library_holds_the_five_things_a_device_keeps() {
    let created = create_s3("five-things").await;
    let dir = LibraryDir::named_here("five-things");

    assert_eq!(created.path, dir.path());
    assert_eq!(
        created.path,
        state_dir()
            .join("libraries")
            .join(created.settings.library_id.to_hex()),
        "the directory is the Library's ID",
    );
    assert_eq!(created.settings.name.as_deref(), Some("five-things"));
    for path in [dir.settings_file(), dir.master_key_file(), dir.index_file()] {
        assert!(path.is_file(), "{} must be a file", path.display());
    }
    assert!(dir.spool_dir().is_dir());
    // An S3 Library references no account, so it holds no envelope for one.
    assert!(!dir.account_envelope_file().exists());
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
    let dir = LibraryDir::named_here("owner-only");

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
    let dir = LibraryDir::named_here("recovery");

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

// A name is the Library's and not a directory's, so a second Library of one
// name is a Library of its own, under its own ID — and neither is chosen for a
// caller naming them: the name finds both, and the refusal names both IDs.
#[tokio::test]
async fn a_second_library_of_one_name_is_a_library_of_its_own() {
    let first = create_s3("one-name-twice").await;
    let key_before = fs::read(
        LibraryDir::of(first.settings.library_id)
            .unwrap()
            .master_key_file(),
    )
    .expect("the key file must be readable");

    let second = create_s3("one-name-twice").await;
    assert_ne!(first.settings.library_id, second.settings.library_id);
    assert_ne!(first.path, second.path);
    assert_eq!(
        fs::read(
            LibraryDir::of(first.settings.library_id)
                .unwrap()
                .master_key_file()
        )
        .expect("the key file must still be readable"),
        key_before,
        "the first Library is left as it was",
    );

    match LibraryDir::find(&LibraryRef::named("one-name-twice")) {
        Err(Error::AmbiguousLibraryName { name, libraries }) => {
            assert_eq!(name, "one-name-twice");
            assert!(libraries.contains(&first.settings.library_id));
            assert!(libraries.contains(&second.settings.library_id));
        }
        other => panic!("a name two Libraries carry must be refused, got {other:?}"),
    }
    for created in [&first, &second] {
        assert_eq!(
            LibraryDir::find(&LibraryRef::Id(created.settings.library_id))
                .expect("each is found by its ID")
                .path(),
            created.path
        );
    }
}

// A directory an interrupted creation left is under the staging suffix, and is
// never taken for a Library: nothing lists it, nothing finds it, and a creation
// beside it succeeds.
#[tokio::test]
async fn a_directory_an_interrupted_creation_left_is_never_a_library() {
    state_dir();
    let leftover = state_dir()
        .join("libraries")
        .join("00000000000000aa.partial");
    fs::create_dir_all(&leftover).expect("the staging directory must be creatable");
    fs::write(leftover.join("settings.json"), b"{}").expect("the leftover must be writable");

    let created = create_s3("beside-a-leftover").await;

    assert!(LibraryDir::on_this_device()
        .expect("the Libraries list")
        .iter()
        .all(|dir| !dir.path().ends_with("00000000000000aa.partial")));
    assert_eq!(
        DeviceSettings::read(&LibraryDir::named_here("beside-a-leftover"))
            .expect("the settings must read"),
        created.settings
    );
}

// FM-15: a name is held to what a person can read — not empty, and no control
// character — and refused before any directory is made, the staging one
// included. Nothing else is asked of it: it names no directory, so a separator
// or a dot is an ordinary character in one.
#[tokio::test]
async fn a_name_nobody_can_read_creates_nothing_and_a_separator_is_a_character() {
    let device = isolated::device();
    for (name, defect) in [
        ("", LibraryNameDefect::Empty),
        ("line\nbreak", LibraryNameDefect::Control),
    ] {
        let mut asked = request("unreadable");
        asked.name = name.to_owned();
        let result = create_library(asked, unasked, |_| ()).await;
        assert!(
            matches!(&result, Err(Error::InvalidLibraryName { defect: found, .. }) if *found == defect),
            "expected {name:?} to be refused, got {result:?}"
        );
    }
    assert!(staged(&device).is_empty(), "nothing was staged");

    let created = create_s3("albums/../2026").await;
    assert_eq!(created.settings.name.as_deref(), Some("albums/../2026"));
    assert_eq!(
        LibraryDir::named_here("albums/../2026").path(),
        created.path
    );
}

/// The directories being staged among the Libraries of `device`.
fn staged(device: &isolated::Device) -> Vec<String> {
    let mut staged: Vec<String> = fs::read_dir(device.path().join("libraries"))
        .map(|listing| {
            listing
                .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
                .filter(|name| name.ends_with(".partial"))
                .collect()
        })
        .unwrap_or_default();
    staged.sort();
    staged
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
    assert!(LibraryDir::find(&LibraryRef::named("nowhere")).is_err());
}

// The Passphrase arrives through a callback, so the caller that was asked is
// where a refusal to give one comes from — a terminal that reached the end of
// its input, or an explorer whose dialog was dismissed. It is not a failure this
// crate can produce, and what it leaves behind is what every other failure of a
// creation leaves: nothing. The directory the Library was being built in is
// removed on the way out, and the name it would have taken is still free.
#[tokio::test]
async fn a_passphrase_that_is_refused_creates_nothing() {
    let device = isolated::device();

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
    assert!(staged(&device).is_empty(), "nothing is left staged");
    assert!(LibraryDir::find(&LibraryRef::named("unasked-for")).is_err());
}

/// What creating a Drive Library called `name` asks for.
fn drive_request(name: &str) -> super::CreateLibraryRequest {
    super::CreateLibraryRequest {
        name: name.to_owned(),
        provider: NewProvider::Drive {
            parent: "stub-parent".to_owned(),
            client_id: CLIENT_ID.to_owned(),
            client_secret: None,
            account: None,
        },
        referencing_passphrase: crate::ReferencingPassphrase::unasked(),
    }
}

// The catalog is the one file of a new Library that the concrete gateway
// creates rather than this crate, and a disk that will not hold it is refused
// at the step that says so. What it leaves is what every other failed creation
// leaves: nothing — no staging directory, and no directory under the name, so
// nothing half made can be mistaken for a Library by the next command.
#[tokio::test]
async fn a_catalog_that_will_not_open_leaves_no_library_behind() {
    let device = isolated::device();

    let result = create_library_through(
        &Reach::this_device().opening_index_with(unopenable_catalog),
        request("no-catalog"),
        passphrase,
        |_| panic!("an S3 Library asks nobody for consent"),
    )
    .await;

    let Err(Error::LibraryNotCreated {
        step,
        orphan_folder,
        cause,
        ..
    }) = &result
    else {
        panic!("expected the creation to stop at the catalog, got {result:?}");
    };
    assert!(matches!(step, CreationStep::Index), "stopped at {step:?}");
    assert!(
        matches!(**cause, Error::Index { .. }),
        "the catalog's own refusal travels as the cause: {cause:?}",
    );
    assert_eq!(
        *orphan_folder, None,
        "an S3 Library has put nothing on Storage before its first head"
    );
    assert!(staged(&device).is_empty(), "nothing is left staged");
    assert!(LibraryDir::find(&LibraryRef::named("no-catalog")).is_err());
}

// A Drive Library, created end to end below the terminal: the consent a person
// gives at a browser, the grant traded for it, and the app folder made under
// the parent they named — every call through the gateway's own request
// building, and none of it reaching Google (spec: SA-1, SA-6, FM-18).
#[tokio::test]
async fn a_drive_library_is_created_through_a_grant_and_a_folder_of_its_own() {
    let device = isolated::device();
    let drive = DriveStub::empty();

    let created = create_library_through(
        &Reach::this_device().reaching_drive_through(Arc::clone(&drive) as _),
        drive_request("on-drive"),
        passphrase,
        consent,
    )
    .await
    .expect("a consent and a Drive that answers are all a Drive Library needs");
    let dir = LibraryDir::named_here("on-drive");

    assert_eq!(
        created.settings.provider,
        ProviderSettings::Drive {
            folder_id: CREATED_FOLDER_ID.to_owned(),
            client_id: CLIENT_ID.to_owned(),
            client_secret: None,
            account: Some("default".to_owned()),
        },
        "the folder Drive minted is the one recorded, and the account left unnamed is `default`",
    );
    assert_eq!(
        drive.folder_named(CREATED_FOLDER_ID),
        Some(created.settings.library_id.app_folder_name()),
        "and it is named for the Library, which is what a join reads it back by",
    );
    assert!(
        device
            .path()
            .join("accounts/default/token-cache.cftc")
            .is_file(),
        "the grant is sealed into the account's directory (spec: SA-8)",
    );
    assert!(
        dir.account_envelope_file().is_file(),
        "and the envelope that opens it into the Library's, not the one it was staged in",
    );
    assert!(!dir.previous_token_cache_file().exists());
    assert_eq!(
        DeviceSettings::read(&dir).expect("the settings this build wrote must read"),
        created.settings,
    );

    // The code traded for the grant, the grant's refresh token traded for the
    // access token the create goes out with, and the folder — nothing else.
    assert_eq!(
        drive.asked(),
        ["token", "token", "create"],
        "{:?}",
        drive.calls(),
    );
}

// And the catalog refused after the folder was made, which is the one creation
// that leaves something behind this crate cannot take back. The refusal names
// the folder, so that whoever reads it knows where to look before trying again.
#[tokio::test]
async fn a_catalog_that_will_not_open_after_the_folder_names_the_folder() {
    let device = isolated::device();

    let result = create_library_through(
        &Reach::this_device()
            .opening_index_with(unopenable_catalog)
            .reaching_drive_through(DriveStub::empty()),
        drive_request("drive-no-catalog"),
        passphrase,
        consent,
    )
    .await;

    assert!(
        matches!(
            &result,
            Err(Error::LibraryNotCreated {
                step: CreationStep::Index,
                orphan_folder: Some(folder),
                ..
            }) if folder == CREATED_FOLDER_ID
        ),
        "expected the folder left on Drive to be named, got {result:?}",
    );
    assert_eq!(
        fs::read_dir(device.path().join("libraries"))
            .map(|listing| listing.count())
            .unwrap_or(0),
        0,
        "nothing is left among the Libraries, staged or whole"
    );
    // And the account consented to for it goes with it: no Library references
    // it, and nothing of it is left behind.
    assert!(
        !device.path().join("accounts/default").exists()
            && !device.path().join("accounts/default.partial").exists(),
    );
}

/// What a created Library's Storage holds, read the way a device recovering it
/// reads it: the listing's control-object names, and a catalog caught up from
/// nothing (spec: RV-1, CK-9).
async fn recovered(store: &dyn ObjectStore, name: &str) -> (Vec<String>, InMemoryIndex) {
    let dir = LibraryDir::named_here(name);
    let unlocked = StoredMasterKeyFile::unlock(&dir, &Passphrase::from_bytes(PASSPHRASE.to_vec()))
        .expect("the Passphrase must open the key");
    let keys = LibraryKeys::derive(&unlocked.master_key, unlocked.epoch);

    let mut names = Vec::new();
    let mut token = None;
    loop {
        let page = store
            .list(token.as_ref())
            .await
            .expect("a store in memory lists");
        names.extend(
            page.objects
                .into_iter()
                .map(|object| object.name)
                .filter(|name| {
                    matches!(
                        ControlObjectName::parse(name),
                        Ok(ControlObjectName::Head { .. })
                    )
                }),
        );
        token = page.next;
        if token.is_none() {
            break;
        }
    }

    let index = InMemoryIndex::new();
    // Nothing is pending on a device that has only just created the Library,
    // so the spool is named and never reached.
    let spool = local_fs();
    catch_up_catalog(CatchUpRequest::new(
        store,
        &index,
        &keys,
        spool.as_ref(),
        crate::device_time::now(),
    ))
    .await
    .expect("a freshly created Library is one to catch up with");
    (names, index)
}

// CP-2, FM-13: creating a Library writes its first head, `head-0` with nothing
// in it, so the first commit consumes a slot a head carries. On a Storage that
// keys objects by name the head persists no slot of its own — the name
// `head-1` is the slot — and a device recovering the Library from nothing
// stands at that head and lists no Entry (spec: RV-1, CK-9).
#[tokio::test]
async fn a_created_library_holds_its_first_head() {
    state_dir();
    let store = Arc::new(InMemoryStore::new(16));

    create_library_through(
        &Reach::this_device().storing_into(Arc::clone(&store) as _),
        request("first-head"),
        passphrase,
        |_| panic!("an S3 Library asks nobody for consent"),
    )
    .await
    .expect("an S3 Library needs nothing but this device and a bucket that answers");

    let (heads, index) = recovered(store.as_ref(), "first-head").await;
    assert_eq!(heads, ["head-0.cfrt"], "the one head is the first one");
    assert_eq!(
        index
            .library_name()
            .await
            .expect("the catalog answers")
            .map(|name| name.into_string()),
        Some("first-head".to_owned()),
        "the first head names the Library (spec: FM-15)",
    );
    let standing = index
        .checkpoint()
        .await
        .expect("the catalog answers")
        .expect("a Library with a head is one a catalog stands somewhere in");
    assert_eq!(standing.head_generation(), Generation::FIRST);
    assert_eq!(
        standing.next_commit_slot(),
        None,
        "where names identify objects the slot is the successor's name, persisted nowhere",
    );
    assert!(
        index
            .containers_under(None)
            .await
            .expect("the catalog answers")
            .is_empty(),
        "a Library just created holds nothing",
    );
}

// The same on Drive's shape of Storage, where the slot `head-0` carries is an
// identifier minted for its successor before the head was written: the one
// every writer of the first commit consumes (spec: CP-2, CP-3).
#[tokio::test]
async fn a_created_drive_library_holds_its_first_head_with_a_minted_slot() {
    let _device = isolated::device();
    let store = Arc::new(InMemoryMintingStore::new(16));

    create_library_through(
        &Reach::this_device()
            .storing_into(Arc::clone(&store) as _)
            .reaching_drive_through(DriveStub::empty()),
        drive_request("first-head-on-drive"),
        passphrase,
        consent,
    )
    .await
    .expect("a consent and a Drive that answers are all a Drive Library needs");

    let (heads, index) = recovered(store.as_ref(), "first-head-on-drive").await;
    assert_eq!(heads, ["head-0.cfrt"], "the one head is the first one");
    let standing = index
        .checkpoint()
        .await
        .expect("the catalog answers")
        .expect("a Library with a head is one a catalog stands somewhere in");
    assert_eq!(standing.head_generation(), Generation::FIRST);
    assert!(
        standing.next_commit_slot().is_some(),
        "where Storage mints identifiers the head carries the one minted for its successor",
    );
}

/// A store that refuses every call, as a provider that will not take this
/// device's writes does.
struct Refusing;

impl Refusing {
    fn refusal() -> coffret_usecase::Error {
        coffret_usecase::Error::PermissionDenied {
            detail: "refused".to_owned(),
            source: None,
        }
    }
}

#[async_trait]
impl ObjectStore for Refusing {
    async fn put(&self, _: &str, _: ByteStream) -> coffret_usecase::Result<UploadedObject> {
        Err(Self::refusal())
    }

    async fn reserve_create(&self, name: &str) -> coffret_usecase::Result<CommitSlot> {
        Ok(CommitSlot::by_name(name))
    }

    async fn put_if_absent(
        &self,
        _: &CommitSlot,
        _: ByteStream,
    ) -> coffret_usecase::Result<ObjectRef> {
        Err(Self::refusal())
    }

    fn object_at(&self, slot: &CommitSlot) -> coffret_usecase::Result<ObjectRef> {
        Ok(ObjectRef::new(slot.name()))
    }

    async fn get(
        &self,
        _: &ObjectRef,
        _: Option<Range<u64>>,
    ) -> coffret_usecase::Result<ByteStream> {
        Err(Self::refusal())
    }

    async fn list(&self, _: Option<&PageToken>) -> coffret_usecase::Result<ObjectPage> {
        Ok(ObjectPage::last(Vec::new()))
    }

    async fn trash(&self, _: &ObjectRef) -> coffret_usecase::Result<()> {
        Err(Self::refusal())
    }

    async fn purge(&self, _: &ObjectRef) -> coffret_usecase::Result<()> {
        Err(Self::refusal())
    }
}

// A first head Storage will not take leaves no Library on this device: the
// step says so, and a Library with no head is not one a later commit may
// start a race for (spec: CP-2). What reached Storage before stays under a
// Library ID nothing references.
#[tokio::test]
async fn a_first_head_storage_refuses_leaves_no_library_behind() {
    let device = isolated::device();

    let result = create_library_through(
        &Reach::this_device().storing_into(Arc::new(Refusing)),
        request("no-first-head"),
        passphrase,
        |_| panic!("an S3 Library asks nobody for consent"),
    )
    .await;

    assert!(
        matches!(
            &result,
            Err(Error::LibraryNotCreated {
                step: CreationStep::FirstHead,
                cause,
                ..
            }) if matches!(**cause, Error::FirstHead { .. })
        ),
        "expected the creation to stop at the first head, got {result:?}",
    );
    assert!(staged(&device).is_empty(), "nothing is left staged");
    assert!(LibraryDir::find(&LibraryRef::named("no-first-head")).is_err());
}
