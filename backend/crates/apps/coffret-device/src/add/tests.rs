//! Taking a dropped file into a mapped folder, and the fence around where it
//! may land.
//!
//! Nothing here reaches Storage: taking a file in writes to a folder and to
//! nothing else, so the cases hand the device an empty catalog with one mapping
//! and read the folder back afterwards.
//!
//! What they are mostly about is the shape of the disk under that mapping. An
//! Entry Path comes from whichever device committed it and says nothing about
//! this one, so a component that is an ordinary folder there may be a symbolic
//! link here — out of the mapped root, or back inside it. A writer that followed
//! one would put somebody's upload where the mappings never pointed, which is
//! what EP-4 and EP-11 exist to refuse. Every case about a link, or about a file
//! standing where a folder must be, therefore asserts twice: what the upload did
//! with the path, *and* that what the upload must not touch is exactly as it
//! was.

use std::path::PathBuf;
use std::sync::Arc;

use coffret_model::{LibraryId, MasterKey, MasterKeyEpoch};
use coffret_usecase::device_state::Mapping;
use coffret_usecase::fetch::FetchError;
use coffret_usecase::{InMemoryIndex, InMemoryStore, Index, LibraryKeys};
use tempfile::TempDir;

use crate::error::Error;
use crate::open_library::OpenLibrary;
use crate::testing::entry_path;

/// What a case drops onto the device.
const DROPPED: &[u8] = b"what somebody dropped onto a folder";

/// What stands outside the mapped root, which no upload may touch.
const OUTSIDE: &[u8] = b"a file of the person's own, outside the Library";

/// What stands inside it where a folder would have to go, which no upload may
/// touch either.
const IN_THE_WAY: &[u8] = b"a file of the person's own, where a folder would go";

/// A device with one Library-root mapping onto an empty folder, and somewhere
/// outside that folder to keep what no upload may reach.
///
/// Both live under one temporary directory that travels with the fixture, so a
/// case that panics leaves nothing behind — and so that the folder outside the
/// mapped root is this case's own rather than a name two cases running at once
/// would share.
struct Device {
    library: OpenLibrary,
    /// The mapped root, which is what the Library's paths stand under.
    root: PathBuf,
    /// A folder beside it, which no Entry Path names.
    elsewhere: PathBuf,
    // Dropped last, removing both.
    _held: TempDir,
}

async fn device() -> Device {
    let held = TempDir::new().expect("a temporary directory must be available");
    let root = held.path().join("mapped");
    let elsewhere = held.path().join("elsewhere");
    for folder in [&root, &elsewhere] {
        std::fs::create_dir_all(folder).expect("making a case's folder must succeed");
    }

    let index = InMemoryIndex::new();
    index
        .set_mapping(Mapping {
            prefix: None,
            local_root: root.clone(),
            // No scan has seen this root yet, so nothing is recorded about the
            // filesystem under it (spec: EP-12).
            root_identity: None,
        })
        .await
        .expect("recording a mapping must succeed");

    let library = OpenLibrary {
        store: Arc::new(InMemoryStore::new(64)),
        index: Arc::new(index),
        keys: LibraryKeys::derive(
            &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
            MasterKeyEpoch::FIRST,
        ),
        spool: std::env::temp_dir(),
        library_id: LibraryId::from_bytes([0x11; LibraryId::BYTE_LEN]),
        epoch: MasterKeyEpoch::FIRST,
        provider: "s3",
    };
    Device {
        library,
        root,
        elsewhere,
        _held: held,
    }
}

impl Device {
    /// A folder outside the mapped root holding one file of the person's own.
    fn secrets(&self) -> PathBuf {
        let secrets = self.elsewhere.join("secrets");
        std::fs::create_dir_all(&secrets).expect("making a folder must succeed");
        std::fs::write(secrets.join("authorized_keys"), OUTSIDE)
            .expect("writing a file must succeed");
        secrets
    }
}

/// Takes one file in, and says what happened.
async fn drop_file(library: &OpenLibrary, path: &str) -> Result<(), Error> {
    let mut incoming = library.receive_file(&entry_path(path)).await?;
    incoming.write(DROPPED).await?;
    incoming.keep().await
}

/// The Entry Path a refusal names, which every case here expects to be its own.
///
/// The blocked folder travels beside it and is what the message names; what
/// these cases are about is that the upload was refused at all, so it is passed
/// over here.
fn refused_path(error: Error) -> String {
    match error {
        Error::Fetch {
            cause: FetchError::UnmaterializablePath { path, .. },
        } => path.as_str().to_owned(),
        other => panic!("the upload must be refused as unmaterializable, and was {other:?}"),
    }
}

/// A folder above the file that is a symbolic link out of the mapped root is
/// refused, and what it points at is untouched.
///
/// The shape is the one another device makes without meaning anything by it:
/// `link/authorized_keys` is an ordinary folder and a file over there, and here
/// `link` happens to be a symbolic link to a folder of the person's own. Writing
/// through it would replace a file the Library has never held and never will,
/// which is exactly the content EP-11 keeps a write from destroying.
#[tokio::test]
async fn a_parent_symlink_out_of_the_root_is_refused() {
    let device = device().await;
    let secrets = device.secrets();
    std::os::unix::fs::symlink(&secrets, device.root.join("link"))
        .expect("making a symbolic link must succeed");

    let refused = drop_file(&device.library, "link/authorized_keys")
        .await
        .expect_err("an upload through a symbolic link must be refused");
    assert_eq!(refused_path(refused), "link/authorized_keys");

    assert_eq!(
        std::fs::read(secrets.join("authorized_keys")).expect("the file outside is still there"),
        OUTSIDE,
        "byte for byte as it was: nothing may be written through the link",
    );
}

/// The same refusal where the link is deeper in the chain rather than first.
///
/// The fence is every component and not the first one: a mapped root whose
/// `albums/` is an ordinary folder and whose `albums/2026` is a link out is the
/// same escape one level down, and a check that only looked at the top-level
/// name would walk straight past it.
#[tokio::test]
async fn a_symlink_deeper_in_the_chain_is_refused() {
    let device = device().await;
    let secrets = device.secrets();
    std::fs::create_dir_all(device.root.join("albums")).expect("making a folder must succeed");
    std::os::unix::fs::symlink(&secrets, device.root.join("albums").join("2026"))
        .expect("making a symbolic link must succeed");

    let refused = drop_file(&device.library, "albums/2026/authorized_keys")
        .await
        .expect_err("an upload through a symbolic link must be refused");
    assert_eq!(refused_path(refused), "albums/2026/authorized_keys");

    assert_eq!(
        std::fs::read(secrets.join("authorized_keys")).expect("the file outside is still there"),
        OUTSIDE,
        "byte for byte as it was, however deep the link was",
    );
}

/// A link pointing back *inside* the mapped root is refused too.
///
/// Nothing escapes here, and it is still not the place the mappings name. EP-9
/// gives an Entry Path one local path on this device, and a file written through
/// a second name for that folder is not at it. A scan walking the root would
/// find the file under its real name and offer it to the Library at a path
/// nobody asked for. Refused rather than quietly redirected, on the
/// no-silent-selection posture EP-4 sets.
#[tokio::test]
async fn a_symlink_pointing_inside_the_root_is_refused() {
    let device = device().await;
    std::fs::create_dir_all(device.root.join("albums")).expect("making a folder must succeed");
    std::os::unix::fs::symlink("albums", device.root.join("pictures"))
        .expect("making a symbolic link must succeed");

    let refused = drop_file(&device.library, "pictures/spring.jpg")
        .await
        .expect_err("an upload through a symbolic link must be refused");
    assert_eq!(refused_path(refused), "pictures/spring.jpg");

    assert!(
        !device.root.join("albums").join("spring.jpg").exists(),
        "the folder the link points at is untouched",
    );
}

/// The file's own name being a symbolic link out of the root does not carry the
/// upload out of it.
///
/// The last component is the one the folders above it are not, and the fence
/// there is the rename rather than the descent: a rename replaces the *name* in
/// the folder the descent left open rather than following what that name points
/// at, so the link is what goes and the person's file on the other side of it is
/// untouched. The upload itself is allowed, because replacing what stands at the
/// path is what taking a file into a mapped folder means once the caller has
/// decided it may be written (spec: EP-11).
#[tokio::test]
async fn a_symlink_at_the_files_own_name_is_replaced_rather_than_followed() {
    let device = device().await;
    let secrets = device.secrets();
    std::os::unix::fs::symlink(
        secrets.join("authorized_keys"),
        device.root.join("authorized_keys"),
    )
    .expect("making a symbolic link must succeed");

    drop_file(&device.library, "authorized_keys")
        .await
        .expect("the upload lands on the name, whatever that name held");

    assert_eq!(
        std::fs::read(secrets.join("authorized_keys")).expect("the file outside is still there"),
        OUTSIDE,
        "byte for byte as it was: nothing may be written through the link",
    );

    let placed = device.root.join("authorized_keys");
    assert!(
        placed
            .symlink_metadata()
            .expect("something stands at the name")
            .is_file(),
        "the name holds the file that was dropped, rather than the link it held",
    );
    assert_eq!(
        std::fs::read(&placed).expect("the file must be readable"),
        DROPPED,
    );
}

/// A component that is an ordinary file rather than a folder is refused the same
/// way a symbolic link is.
///
/// `O_DIRECTORY` is the other half of what each step of the descent asks for,
/// and what it answers is a verdict EP-4 already has: no file on this device can
/// stand for the path, a name on the way to it being occupied by something that
/// is not a folder. Reported as that rather than as whatever the operating
/// system said about creating a directory — and the file in the way is left
/// exactly as it is.
#[tokio::test]
async fn an_ordinary_file_where_a_folder_must_be_is_refused() {
    let device = device().await;
    std::fs::write(device.root.join("albums"), IN_THE_WAY).expect("writing a file must succeed");

    let refused = drop_file(&device.library, "albums/spring.jpg")
        .await
        .expect_err("an upload under an ordinary file must be refused");
    assert_eq!(refused_path(refused), "albums/spring.jpg");

    assert_eq!(
        std::fs::read(device.root.join("albums")).expect("the file is still there"),
        IN_THE_WAY,
        "the file that stood where a folder had to go is untouched",
    );
}

/// The ordinary case is unchanged: real folders all the way down, and the file
/// lands where the mappings say.
///
/// Including the folders that were not there yet — an Entry Path's separators
/// are the whole of what a folder is, so the descent makes them (spec: EP-2).
#[tokio::test]
async fn an_ordinary_chain_of_folders_takes_the_file() {
    let device = device().await;

    drop_file(&device.library, "albums/2026/spring.jpg")
        .await
        .expect("an upload into real folders must succeed");

    let placed = device.root.join("albums").join("2026").join("spring.jpg");
    assert_eq!(
        std::fs::read(&placed).expect("the file must be where the mappings say"),
        DROPPED,
    );
}
