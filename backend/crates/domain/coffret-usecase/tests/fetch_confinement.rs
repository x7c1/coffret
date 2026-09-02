//! Where a fetch may put a file, when the disk under a mapped root is not the
//! shape the Entry Path assumed.
//!
//! Not part of the fetch conformance suite, and deliberately: what these cases
//! are about is a local filesystem rather than a Storage backend, and the answer
//! is the same whichever provider the Library is on. So they run against the
//! in-memory store alone, and the thing under test is the folder afterwards.
//!
//! The shape they all arrange is one no malice is needed to produce. An Entry
//! Path comes from whichever device committed it and says nothing about this
//! one: `link/authorized_keys` is an ordinary folder and a file over there, and
//! here `link` happens to be a symbolic link to a folder of the person's own.
//! Following it would place bytes where the mappings never pointed, and — worse
//! — could replace a file the Library has never held. EP-4 refuses a path this
//! device cannot materialize rather than inventing a place for it, and EP-11
//! places bytes only where the device can vouch for what is there; the scan side
//! already keeps the mirror of the rule by not following symbolic links (EP-8).
//!
//! Every case about a link, or about a file standing where a folder must be,
//! therefore asserts twice: what the run did with the Entry, *and* that the file
//! it must not touch is byte for byte as it was.
//!
//! What the run does about it is report and carry on. The shape of one folder is
//! a fact about one Entry — the device that committed the path had ordinary
//! folders all the way down — so the Entry is a finding and every other one is
//! placed, which is the same posture EP-11 already gives a path a fetch may not
//! write at for any other reason. The last case is what holds the run to it: a
//! Library with a blocked Entry and an unrelated one, and only the blocked one
//! goes unplaced.

use std::path::{Path, PathBuf};

use coffret_model::{EntryPath, MasterKey, MasterKeyEpoch};
use coffret_usecase::commit::CommitPolicy;
use coffret_usecase::device_state::{BatchId, DeviceTime, Mapping};
use coffret_usecase::fetch::{
    fetch_folders, FetchError, FetchOutcome, FetchRequest, LibraryKeys, Surfaced,
};
use coffret_usecase::sync::{sync_folders, SyncRequest};
use coffret_usecase::{InMemoryIndex, InMemoryStore, Index};
use tempfile::TempDir;

/// What the source device puts in the Library.
const HELD: &[u8] = b"what the Library holds";

/// What stands outside the mapped root, which no fetch may touch.
const OUTSIDE: &[u8] = b"a file of the person's own, outside the Library";

/// What stands inside it where a folder would have to go, which no fetch may
/// touch either.
const IN_THE_WAY: &[u8] = b"a file of the person's own, where a folder would go";

/// A threshold no case reaches by committing.
const NEVER_CHECKPOINT: u64 = 1_000;

/// Two devices over one store, and somewhere outside the fetching device's
/// mapped root to keep what no run may reach.
///
/// Two devices because that is what a fetch is worth testing over: the catalogs
/// share nothing, so the fetching device's catch-up is a real restore-and-replay
/// (spec: CK-9, RV-1). Everything lives under one temporary directory that
/// travels with the fixture, so a case that panics leaves nothing behind.
struct Devices {
    store: InMemoryStore,
    source: InMemoryIndex,
    source_folder: PathBuf,
    target: InMemoryIndex,
    /// The fetching device's mapped root.
    root: PathBuf,
    /// A folder beside it, which no Entry Path names.
    elsewhere: PathBuf,
    spool: PathBuf,
    _held: TempDir,
}

/// Everything one epoch's Containers are sealed and opened with.
fn keys() -> LibraryKeys {
    LibraryKeys::derive(
        &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
        MasterKeyEpoch::FIRST,
    )
}

/// The clock the fixture's `run`th operation runs at.
fn at(run: i64) -> DeviceTime {
    DeviceTime::from_unix_seconds(1_700_000_000 + run)
}

/// A policy that keeps a case's Library small and its checkpoints out of the
/// way.
fn policy() -> CommitPolicy {
    CommitPolicy::default().with_checkpoint_threshold(NEVER_CHECKPOINT)
}

impl Devices {
    /// An empty Library, two empty catalogs, and the folders around them.
    async fn new() -> Self {
        let held = TempDir::new().expect("a temporary directory must be available");
        let source_folder = held.path().join("source");
        let root = held.path().join("mapped");
        let elsewhere = held.path().join("elsewhere");
        let spool = held.path().join("spool");
        for folder in [&source_folder, &root, &elsewhere, &spool] {
            std::fs::create_dir_all(folder).expect("making a case's folder must succeed");
        }

        let devices = Self {
            store: InMemoryStore::new(8),
            source: InMemoryIndex::new(),
            source_folder,
            target: InMemoryIndex::new(),
            root,
            elsewhere,
            spool,
            _held: held,
        };
        map(&devices.source, &devices.source_folder).await;
        map(&devices.target, &devices.root).await;
        devices
    }

    /// Puts one file in the Library at `path`, from the source device.
    async fn commit(&self, path: &str) {
        self.commit_all(&[path]).await
    }

    /// The same for several files, which one sync carries in together.
    ///
    /// One sync rather than one each, because a case about what a run does with
    /// the *rest* of the Library wants the blocked Entry and its neighbour in
    /// one Container: a run that placed the neighbour only because it lived
    /// somewhere else entirely would prove less (spec: PK-16).
    async fn commit_all(&self, paths: &[&str]) {
        for path in paths {
            write(&self.source_folder, path, HELD);
        }
        sync_folders(
            SyncRequest::new(
                &self.store,
                &self.source,
                &keys(),
                &self.spool,
                BatchId::new("run-1"),
                at(1),
            )
            .with_policy(policy()),
        )
        .await
        .unwrap_or_else(|error| panic!("a sync of the source folder must succeed: {error}"));
    }

    /// One fetch into the mapped root.
    ///
    /// The whole outcome rather than the placed paths alone: a case about what a
    /// run declined has to read what it reported, and a run that placed nothing
    /// and said nothing about it would pass a case that only counted files
    /// (spec: EP-11).
    async fn fetch(&self) -> Result<FetchOutcome, FetchError> {
        let keys = keys();
        fetch_folders(
            FetchRequest::new(&self.store, &self.target, &keys, at(2)).with_policy(policy()),
        )
        .await
    }

    /// A folder outside the mapped root holding one file of the person's own.
    fn secrets(&self) -> PathBuf {
        let secrets = self.elsewhere.join("secrets");
        std::fs::create_dir_all(&secrets).expect("making a folder must succeed");
        std::fs::write(secrets.join("authorized_keys"), OUTSIDE)
            .expect("writing a file must succeed");
        secrets
    }
}

/// Maps a device's folder onto the whole Library (spec: EP-9).
async fn map(index: &InMemoryIndex, local_root: &Path) {
    index
        .set_mapping(Mapping {
            prefix: None,
            local_root: local_root.to_path_buf(),
            // No scan has seen this root yet, so nothing is recorded about the
            // filesystem under it (spec: EP-12).
            root_identity: None,
        })
        .await
        .expect("recording a mapping must succeed");
}

/// Writes a file under a folder, making the folders above it.
fn write(folder: &Path, relative: &str, content: &[u8]) {
    let path = folder.join(relative);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("making a folder must succeed");
    }
    std::fs::write(&path, content).expect("writing a file must succeed");
}

/// The one finding a run made: the Entry Path it is about, and the folder on
/// this device the descent stopped at.
///
/// Exactly one, because every case here arranges exactly one thing in the way —
/// and a run that reported two would be reporting something no case asked for.
/// The folder is read as well as the path because it is what a person acts on:
/// the Entry Path says which file went unplaced, and this says which name to
/// look at.
fn only_unreachable(outcome: &FetchOutcome) -> (&str, &Path) {
    match outcome.surfaced.as_slice() {
        [Surfaced::UnreachablePlace { path, component }] => (path.as_str(), component.as_path()),
        other => {
            panic!("the Entry must be surfaced as unreachable, and the run reported {other:?}")
        }
    }
}

/// A folder above the file that is a symbolic link out of the mapped root is
/// refused, and what it points at is untouched.
///
/// This is the case the confinement exists for. The file the link leads to is
/// the person's own, the Library has never held it, and a placement that
/// followed the link would replace it with an Entry from somewhere else
/// entirely.
#[tokio::test]
async fn a_parent_symlink_out_of_the_root_is_refused() {
    let devices = Devices::new().await;
    devices.commit("link/authorized_keys").await;

    let secrets = devices.secrets();
    let blocked = devices.root.join("link");
    std::os::unix::fs::symlink(&secrets, &blocked).expect("making a symbolic link must succeed");

    let outcome = devices.fetch().await.expect("the run itself must finish");
    assert!(
        outcome.fetched.is_empty(),
        "nothing may be placed through a symbolic link",
    );
    assert_eq!(
        only_unreachable(&outcome),
        ("link/authorized_keys", blocked.as_path()),
        "and the link is named, because it is what there is to look at",
    );

    assert_eq!(
        std::fs::read(secrets.join("authorized_keys")).expect("the file outside is still there"),
        OUTSIDE,
        "byte for byte as it was: nothing may be written through the link",
    );
}

/// The same refusal where the link is deeper in the chain rather than first.
///
/// The fence is every component and not the first one: a check that looked only
/// at the top-level name would walk straight past a mapped root whose `albums/`
/// is an ordinary folder and whose `albums/2026` is a link out of it.
#[tokio::test]
async fn a_symlink_deeper_in_the_chain_is_refused() {
    let devices = Devices::new().await;
    devices.commit("albums/2026/authorized_keys").await;

    let secrets = devices.secrets();
    std::fs::create_dir_all(devices.root.join("albums")).expect("making a folder must succeed");
    let blocked = devices.root.join("albums").join("2026");
    std::os::unix::fs::symlink(&secrets, &blocked).expect("making a symbolic link must succeed");

    let outcome = devices.fetch().await.expect("the run itself must finish");
    assert!(
        outcome.fetched.is_empty(),
        "nothing may be placed through a symbolic link, however deep it is",
    );
    assert_eq!(
        only_unreachable(&outcome),
        ("albums/2026/authorized_keys", blocked.as_path()),
        "and it is the link that is named, not the ordinary folder above it",
    );

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
/// a second name for that folder is not at it — a scan walking the root
/// afterwards would find the file under its real name and offer it to the
/// Library at a path nobody asked for. Refused rather than quietly redirected,
/// on the no-silent-selection posture EP-4 sets.
#[tokio::test]
async fn a_symlink_pointing_inside_the_root_is_refused() {
    let devices = Devices::new().await;
    devices.commit("pictures/spring.jpg").await;

    std::fs::create_dir_all(devices.root.join("albums")).expect("making a folder must succeed");
    let blocked = devices.root.join("pictures");
    std::os::unix::fs::symlink("albums", &blocked).expect("making a symbolic link must succeed");

    let outcome = devices.fetch().await.expect("the run itself must finish");
    assert!(
        outcome.fetched.is_empty(),
        "a second name for a folder of the root is not the place the mappings name",
    );
    assert_eq!(
        only_unreachable(&outcome),
        ("pictures/spring.jpg", blocked.as_path()),
    );

    assert!(
        !devices.root.join("albums").join("spring.jpg").exists(),
        "the folder the link points at is untouched",
    );
}

/// The file's own name being a symbolic link is not an empty place either.
///
/// The last component is the one the folders above it are not, and it is where a
/// link is likeliest to stand — the shape the whole confinement is about,
/// `link/authorized_keys`, ends in exactly such a name on the device that
/// committed it. A run that asked what stood there *through* the link would read
/// the person's own file, and — the file being nothing this device ever
/// materialized — would either report on a file outside the root or, had the
/// link been dangling, decide the place was free and write through it. The look
/// stats the name itself instead (spec: EP-8): a link is something in the way,
/// so the Entry is reported and nothing is written.
#[tokio::test]
async fn a_symlink_at_the_files_own_name_is_not_an_empty_place() {
    let devices = Devices::new().await;
    devices.commit("authorized_keys").await;

    let secrets = devices.secrets();
    std::os::unix::fs::symlink(
        secrets.join("authorized_keys"),
        devices.root.join("authorized_keys"),
    )
    .expect("making a symbolic link must succeed");

    let outcome = devices.fetch().await.expect("the run itself must finish");
    assert!(
        outcome.fetched.is_empty(),
        "a link standing at the path is something in the way, so nothing is placed",
    );
    assert_eq!(
        outcome.surfaced,
        vec![Surfaced::ForeignFile {
            path: EntryPath::nfc("authorized_keys"),
        }],
        "and it is reported rather than passed over in silence",
    );

    assert_eq!(
        std::fs::read(secrets.join("authorized_keys")).expect("the file outside is still there"),
        OUTSIDE,
        "byte for byte as it was: nothing may be written through the link",
    );
}

/// A component that is an ordinary file rather than a folder is refused the same
/// way a symbolic link is.
///
/// `O_DIRECTORY` is the other half of what each step of the descent asks for,
/// and it answers a question EP-4 already has a verdict for: no file on this
/// device can stand for the Entry Path, because a name on the way to it is
/// occupied by something that is not a folder. Reported as that rather than as
/// whatever the operating system said about creating a directory.
#[tokio::test]
async fn an_ordinary_file_where_a_folder_must_be_is_refused() {
    let devices = Devices::new().await;
    devices.commit("albums/spring.jpg").await;

    let blocked = devices.root.join("albums");
    std::fs::write(&blocked, IN_THE_WAY).expect("writing a file must succeed");

    let outcome = devices.fetch().await.expect("the run itself must finish");
    assert!(
        outcome.fetched.is_empty(),
        "a name that is not a folder is not one to make a folder of",
    );
    assert_eq!(
        only_unreachable(&outcome),
        ("albums/spring.jpg", blocked.as_path()),
    );

    assert_eq!(
        std::fs::read(&blocked).expect("the file is still there"),
        IN_THE_WAY,
        "the file that stood where a folder had to go is untouched",
    );
}

/// One blocked Entry costs its own file and nothing else.
///
/// The blast radius of a folder with the wrong shape, which is the whole reason
/// this is a finding rather than a failure. A person whose mapped root holds one
/// `Documents -> /Volumes/big/Documents` has done nothing unusual, and a run
/// that placed not one file of their Library over it would be answering a local
/// shape with a refusal of everything (spec: EP-11, and the posture EP-4 sets).
///
/// Both Entries are in one Container, so the unrelated file is placed out of the
/// very same fetch the blocked one was selected out of.
#[tokio::test]
async fn a_blocked_entry_does_not_cost_the_rest_of_the_run() {
    let devices = Devices::new().await;
    devices
        .commit_all(&["albums/spring.jpg", "link/authorized_keys"])
        .await;

    let secrets = devices.secrets();
    let blocked = devices.root.join("link");
    std::os::unix::fs::symlink(&secrets, &blocked).expect("making a symbolic link must succeed");

    let outcome = devices.fetch().await.expect("the run itself must finish");
    assert_eq!(
        outcome.fetched,
        vec![EntryPath::nfc("albums/spring.jpg")],
        "the Entry the link says nothing about is placed",
    );
    assert_eq!(
        only_unreachable(&outcome),
        ("link/authorized_keys", blocked.as_path()),
        "and the one it does say something about is reported, naming the link",
    );

    assert_eq!(
        std::fs::read(devices.root.join("albums").join("spring.jpg"))
            .expect("the placed file must be where the mappings say"),
        HELD,
    );
    assert_eq!(
        std::fs::read(secrets.join("authorized_keys")).expect("the file outside is still there"),
        OUTSIDE,
        "byte for byte as it was: carrying on is not carrying on through the link",
    );
}

/// The ordinary case is unchanged: real folders all the way down, and the file
/// lands where the mappings say.
///
/// Including the folders that were not there yet — an Entry Path's separators
/// are the whole of what a folder is, so the descent makes them (spec: EP-2).
#[tokio::test]
async fn an_ordinary_chain_of_folders_places_the_file() {
    let devices = Devices::new().await;
    devices.commit("albums/2026/spring.jpg").await;

    let outcome = devices.fetch().await.expect("a fetch into real folders");
    assert_eq!(
        outcome.fetched,
        vec![EntryPath::nfc("albums/2026/spring.jpg")]
    );

    let placed = devices.root.join("albums").join("2026").join("spring.jpg");
    assert_eq!(
        std::fs::read(&placed).expect("the file must be where the mappings say"),
        HELD,
    );
}
