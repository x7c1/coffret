//! Opening a Library: the name it carries, and the move of a directory an
//! earlier build named after it (spec: FM-15, EL-1).

use std::fs;

use coffret_format::{generate_library_id, generate_master_key, StoredMasterKey};
use coffret_logging::testing::CapturedLogs;
use coffret_model::{LibraryId, MasterKeyEpoch, Passphrase};
use coffret_sqlite_index::SqliteIndex;
use coffret_usecase::catch_up::{catch_up_catalog, CatchUpRequest};
use coffret_usecase::commit::{
    commit_batch, write_first_head, CommitPolicy, CommitRequest, ControlKeys, PreparedBatch,
};
use coffret_usecase::{InMemoryIndex, LibraryKeys, ObjectStore};
use zeroize::Zeroizing;

use super::open_library;
use crate::device_settings::{DeviceSettings, ProviderSettings};
use crate::join_library::{join_library, JoinLibraryRequest, JoinedProvider};
use crate::library_dir::{isolated, LibraryDir};
use crate::server_lock::ServerLock;
use crate::stored_master_key_file::StoredMasterKeyFile;
use crate::testing::{create_elsewhere, create_s3, passphrase, stub_bucket, PASSPHRASE, REGION};
use crate::{owner_only, s3, LibraryRef};

/// Puts on this device, in `libraries/<directory>`, a Library as a build
/// before Library names left it: its first head names nothing, and the
/// directory is named after what this device called it.
///
/// Built the way that build built one — a Master Key under the Passphrase, an
/// empty catalog, a spool and the settings — over a first head on the stub
/// bucket written without a name.
async fn made_before_names(directory: &str) -> LibraryId {
    made_before_names_with_keys(directory).await.0
}

/// [`made_before_names`], handing back the keys and the store too, for a case
/// that writes into the Library the way another device would.
async fn made_before_names_with_keys(
    directory: &str,
) -> (LibraryId, LibraryKeys, std::sync::Arc<dyn ObjectStore>) {
    let library_id = generate_library_id().expect("an ID is drawn");
    let prefix = library_id
        .app_prefix("archive/")
        .expect("the base ends in a separator");
    let store = s3::store("photos", &prefix, Some(stub_bucket()), Some(REGION), true).await;
    let master_key = generate_master_key().expect("a Master Key is drawn");
    let epoch = MasterKeyEpoch::FIRST;
    write_first_head(
        store.as_ref(),
        &ControlKeys::derive(&master_key, epoch),
        &CommitPolicy::default(),
        None,
    )
    .await
    .expect("the stub bucket takes a first head");

    let dir = LibraryDir::beside(directory).expect("the state directory is set");
    owner_only::create_dir(dir.path()).expect("the directory is made");
    StoredMasterKeyFile::write(
        &dir,
        &StoredMasterKey::create(
            &Passphrase::from_bytes(PASSPHRASE.to_vec()),
            &master_key,
            epoch,
        )
        .expect("the Master Key is stored"),
    )
    .expect("the stored form is written");
    SqliteIndex::open(dir.index_file()).expect("the catalog is made");
    owner_only::create_dir(&dir.spool_dir()).expect("the spool is made");
    DeviceSettings::new(
        library_id,
        ProviderSettings::S3 {
            bucket: "photos".to_owned(),
            prefix,
            endpoint: Some(stub_bucket().to_owned()),
            region: Some(REGION.to_owned()),
            path_style: true,
        },
    )
    .write(&dir)
    .expect("the settings are written");
    (library_id, LibraryKeys::derive(&master_key, epoch), store)
}

/// The heads the Library `library_id` holds on the stub bucket.
async fn heads(library_id: LibraryId) -> Vec<String> {
    let prefix = library_id
        .app_prefix("archive/")
        .expect("the base ends in a separator");
    let store = s3::store("photos", &prefix, Some(stub_bucket()), Some(REGION), true).await;
    let page = store.list(None).await.expect("the stub bucket lists");
    let mut heads: Vec<String> = page
        .objects
        .into_iter()
        .map(|object| object.name)
        .filter(|name| name.starts_with("head-"))
        .collect();
    heads.sort();
    heads
}

// FM-15: a Library an earlier build put here is given the name of the
// directory it was kept in — a record carrying that name and nothing else —
// and moved to the directory its ID names. It is found by that name before
// and after the move.
#[tokio::test]
async fn a_library_made_before_names_is_named_after_its_directory_and_moved() {
    let device = isolated::device();
    let library_id = made_before_names("old").await;
    let old = device.path().join("libraries").join("old");
    let moved = device.path().join("libraries").join(library_id.to_hex());
    assert_eq!(
        LibraryDir::find(&LibraryRef::named("old"))
            .expect("found by the directory's name before it moves")
            .path(),
        old
    );

    let opened = open_library(&LibraryRef::named("old"), passphrase)
        .await
        .expect("a Library an earlier build put here opens");

    assert_eq!(opened.name.as_ref().map(|name| name.as_str()), Some("old"));
    assert!(
        !old.exists(),
        "the directory an earlier build named is gone"
    );
    assert!(moved.is_dir(), "and the Library is under its ID");
    assert_eq!(opened.spool, moved.join("spool"));
    let dir = LibraryDir::find(&LibraryRef::named("old")).expect("found by name after the move");
    assert_eq!(dir.path(), moved);
    assert_eq!(
        DeviceSettings::read(&dir)
            .expect("the settings read")
            .name
            .as_deref(),
        Some("old"),
        "the settings keep a copy of the name",
    );
    assert_eq!(
        opened
            .index
            .library_name()
            .await
            .expect("the catalog answers")
            .map(|name| name.into_string()),
        Some("old".to_owned())
    );
    assert_eq!(
        heads(library_id).await,
        ["head-0.cfrt", "head-1.cfrt"],
        "the name is one record, carrying nothing else"
    );
    drop(opened);

    // Opened again, nothing more is committed: the catalog names it.
    open_library(&LibraryRef::Id(library_id), passphrase)
        .await
        .expect("the moved Library opens by its ID");
    assert_eq!(heads(library_id).await, ["head-0.cfrt", "head-1.cfrt"]);
}

// LA-8: while a server holds the Library its directory is not moved from under
// it; the Library is named all the same, and moved by the next open no server
// holds it through.
#[tokio::test]
async fn a_library_a_server_holds_is_named_and_moved_later() {
    let device = isolated::device();
    let library_id = made_before_names("held").await;
    let held = device.path().join("libraries").join("held");
    let moved = device.path().join("libraries").join(library_id.to_hex());

    let hold = ServerLock::take(&LibraryDir::find(&LibraryRef::named("held")).unwrap())
        .expect("the lock is free");
    let opened = open_library(&LibraryRef::named("held"), passphrase)
        .await
        .expect("a held Library opens");
    assert_eq!(opened.name.as_ref().map(|name| name.as_str()), Some("held"));
    assert!(held.is_dir(), "a held Library is left where it is");
    assert!(!moved.exists());
    assert_eq!(
        DeviceSettings::read(&LibraryDir::find(&LibraryRef::named("held")).unwrap())
            .expect("the settings read")
            .name
            .as_deref(),
        Some("held"),
    );
    drop(opened);
    drop(hold);

    open_library(&LibraryRef::named("held"), passphrase)
        .await
        .expect("the Library opens once nothing holds it");
    assert!(!held.exists());
    assert!(moved.is_dir(), "and is moved then");
    assert_eq!(
        heads(library_id).await,
        ["head-0.cfrt", "head-1.cfrt"],
        "named once, not again"
    );
}

// FM-15, LA-8: a server moves a Library an earlier build put here before it
// unlocks it, since it holds the Library from then on. The directory's name is
// kept in the settings, apart from the copy of the Library's name, and the
// unlock that follows — under the server's own hold — gives the Library that
// name and drops it.
#[tokio::test]
async fn a_library_a_server_moves_before_unlocking_it_is_named_after_its_directory() {
    let device = isolated::device();
    let library_id = made_before_names("served").await;
    let old = device.path().join("libraries").join("served");
    let moved = device.path().join("libraries").join(library_id.to_hex());

    // What a server does before it takes its own lock.
    let dir = crate::move_to_its_id(LibraryDir::find(&LibraryRef::named("served")).unwrap())
        .expect("the settings read");
    assert_eq!(dir.path(), moved);
    assert!(
        !old.exists(),
        "the directory an earlier build named is gone"
    );
    let settings = DeviceSettings::read(&dir).expect("the settings read");
    assert_eq!(settings.earlier_name.as_deref(), Some("served"));
    assert_eq!(
        settings.name, None,
        "nothing is copied but from the catalog"
    );
    assert_eq!(
        LibraryDir::find(&LibraryRef::named("served"))
            .expect("found by the earlier name after the move")
            .path(),
        moved
    );
    assert_eq!(
        LibraryDir::libraries()
            .expect("the libraries directory lists")
            .into_iter()
            .map(|library| library.name)
            .collect::<Vec<_>>(),
        [Some("served".to_owned())],
        "listed under the earlier name until it is named"
    );
    assert_eq!(
        heads(library_id).await,
        ["head-0.cfrt"],
        "nothing committed yet"
    );

    // The server's own hold, across its unlock.
    let hold = ServerLock::take(&dir).expect("the lock is free once the move is done");
    let opened = open_library(&LibraryRef::named("served"), passphrase)
        .await
        .expect("the moved Library opens");
    assert_eq!(
        opened.name.as_ref().map(|name| name.as_str()),
        Some("served")
    );
    assert_eq!(opened.spool, moved.join("spool"));
    assert_eq!(
        heads(library_id).await,
        ["head-0.cfrt", "head-1.cfrt"],
        "the name is one record, carrying nothing else"
    );
    let settings = DeviceSettings::read(&dir).expect("the settings read");
    assert_eq!(settings.name.as_deref(), Some("served"));
    assert_eq!(settings.earlier_name, None, "the catalog names it now");
    drop(opened);
    drop(hold);
}

// LA-8: a server starting while another holds the Library moves nothing and
// writes nothing; the lock it then takes is what refuses it.
#[tokio::test]
async fn a_library_another_server_holds_is_not_moved_before_an_unlock() {
    let device = isolated::device();
    let library_id = made_before_names("busy").await;
    let old = device.path().join("libraries").join("busy");

    let found = LibraryDir::find(&LibraryRef::named("busy")).unwrap();
    let hold = ServerLock::take(&found).expect("the lock is free");
    let dir = crate::move_to_its_id(found).expect("the settings read");
    assert_eq!(dir.path(), old, "a held Library is left where it is");
    assert!(!device
        .path()
        .join("libraries")
        .join(library_id.to_hex())
        .exists());
    assert_eq!(
        DeviceSettings::read(&dir)
            .expect("the settings read")
            .earlier_name,
        None,
        "nothing is written while another holds it"
    );
    assert!(matches!(
        ServerLock::take(&dir),
        Err(crate::Error::LibraryAlreadyServed { .. })
    ));
    drop(hold);
}

// EL-1: the Library's name is something a person chose, so no diagnostic
// event carries it — not as it is created, joined, opened, or given to a
// Library an earlier build put here, whether or not a server moved it first.
#[tokio::test]
async fn no_event_carries_the_library_name() {
    const SENTINEL: &str = "sentinel-name-7f3a9c";
    const EARLIER: &str = "sentinel-dir-41b2d8";
    const MOVED: &str = "sentinel-moved-c05e17";
    let _device = isolated::device();
    let logs = CapturedLogs::capture();

    let created = create_elsewhere(SENTINEL).await;
    let ProviderSettings::S3 { prefix, .. } = &created.settings.provider else {
        panic!("an S3 Library is recorded as one");
    };
    join_library(
        JoinLibraryRequest {
            provider: JoinedProvider::S3 {
                bucket: "photos".to_owned(),
                prefix: prefix.clone(),
                endpoint: Some(stub_bucket().to_owned()),
                region: Some(REGION.to_owned()),
                path_style: true,
            },
            referencing_passphrase: crate::ReferencingPassphrase::unasked(),
        },
        || Ok(Zeroizing::new(created.recovery_code.to_grouped_string())),
        passphrase,
        |_| panic!("an S3 Library asks nobody for consent"),
    )
    .await
    .expect("the Library is joined");
    open_library(&LibraryRef::named(SENTINEL), passphrase)
        .await
        .expect("the joined Library opens by its name");
    let _ = open_library(&LibraryRef::named("nothing-by-this-name"), passphrase).await;

    made_before_names(EARLIER).await;
    open_library(&LibraryRef::named(EARLIER), passphrase)
        .await
        .expect("a Library an earlier build put here opens");
    // And one a server moves before it unlocks it.
    made_before_names(MOVED).await;
    crate::move_to_its_id(LibraryDir::find(&LibraryRef::named(MOVED)).unwrap())
        .expect("the settings read");
    open_library(&LibraryRef::named(MOVED), passphrase)
        .await
        .expect("a Library moved before its unlock opens");
    create_s3(SENTINEL).await;
    let _ = open_library(&LibraryRef::named(SENTINEL), passphrase).await;

    assert!(!logs.events().is_empty(), "the flows recorded something");
    logs.assert_free_of(&[SENTINEL, EARLIER, MOVED]);
}

// The settings file a device lists its Libraries from is not the Library's:
// a copy of the name that drifted from the catalog's is put back at the next
// open (spec: FM-15).
#[tokio::test]
async fn an_open_puts_the_catalog_s_name_back_into_the_settings() {
    let _device = isolated::device();
    let created = create_s3("the-catalog-s-name").await;
    let dir = LibraryDir::named_here("the-catalog-s-name");
    let mut settings = DeviceSettings::read(&dir).expect("the settings read");
    settings.name = Some("a-stale-copy".to_owned());
    settings.write(&dir).expect("the settings are written");

    open_library(&LibraryRef::named("a-stale-copy"), passphrase)
        .await
        .expect("the Library opens by the copy it has");
    assert_eq!(
        DeviceSettings::read(&dir)
            .expect("the settings read")
            .name
            .as_deref(),
        Some("the-catalog-s-name")
    );
    assert!(fs::metadata(&created.path).is_ok());
}

// FM-15: a catalog an earlier build kept may have replayed a record another
// device wrote naming the Library, and kept nothing of the name. So before a
// Library in a directory an earlier build named is given that directory's
// name, its records are read afresh: the name they give is the Library's, and
// it is the one committed and kept, not the directory's.
#[tokio::test]
async fn a_name_an_earlier_catalog_did_not_keep_is_read_from_storage() {
    let device = isolated::device();
    let (library_id, keys, store) = made_before_names_with_keys("kept-without").await;
    let given = coffret_model::LibraryName::parse("given-elsewhere").expect("a name");
    commit_batch(CommitRequest::new(
        store.as_ref(),
        &InMemoryIndex::new(),
        keys.control(),
        PreparedBatch::default().naming(given.clone()),
    ))
    .await
    .expect("another device names the Library");

    // This device's catalog, as an earlier build left it: at the head the
    // naming record made, with no name in it.
    let dir = device.path().join("libraries").join("kept-without");
    {
        let index = SqliteIndex::open(dir.join("index.sqlite")).expect("the catalog opens");
        catch_up_catalog(CatchUpRequest::new(
            store.as_ref(),
            &index,
            &keys,
            &coffret_local_fs::UnixFs::new(),
            crate::device_time::now(),
        ))
        .await
        .expect("the catalog catches up");
    }
    rusqlite::Connection::open(dir.join("index.sqlite"))
        .expect("the catalog file opens")
        .execute("UPDATE checkpoint SET library_name = NULL", [])
        .expect("the name is taken out, as an earlier build never kept it");

    let opened = open_library(&LibraryRef::named("kept-without"), passphrase)
        .await
        .expect("the Library opens");
    assert_eq!(opened.name.as_ref(), Some(&given));
    assert!(!dir.exists(), "and it is moved to its ID");
    assert!(
        !device
            .path()
            .join("libraries")
            .join(library_id.to_hex())
            .join("naming")
            .exists(),
        "the catalog made to read the name is gone"
    );
    assert_eq!(
        DeviceSettings::read(&LibraryDir::find(&LibraryRef::Id(library_id)).unwrap())
            .expect("the settings read")
            .name
            .as_deref(),
        Some("given-elsewhere")
    );
    assert_eq!(
        opened
            .index
            .library_name()
            .await
            .expect("the catalog answers"),
        Some(given),
        "and the catalog keeps it now"
    );
}
