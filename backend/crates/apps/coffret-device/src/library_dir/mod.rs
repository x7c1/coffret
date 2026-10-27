use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use coffret_model::LibraryId;
use coffret_usecase::{LocalIoError, LocalOperation};

use crate::error::{Error, Result};

// Which whole Libraries are on this device, and the one a name or an ID picks
// out of them.
mod find;

/// Names the directory Libraries are kept under, in place of the default one.
///
/// The default is the state directory the platform names — `$XDG_STATE_HOME`,
/// or `$HOME/.local/state` where that is unset — with `coffret` under it, which
/// is the directory the log files already live in. This variable moves the
/// Libraries under it, which is what lets a test run against a directory of its
/// own rather than against the state of whoever started it.
///
/// It does not move the log files: those are the logging crate's, and follow
/// `COFFRET_LOG_DIR`. A run that has to leave nothing behind anywhere sets both
/// — this one alone leaves the log where whoever started the run keeps theirs.
pub const STATE_DIRECTORY: &str = "COFFRET_STATE_DIR";

/// What a Library directory is called while it is still being built.
///
/// A creation that is interrupted leaves one of these rather than half a
/// Library: the directory only takes its real name once everything in it is
/// written, so a directory under the real name is always a whole Library.
pub(crate) const STAGING_SUFFIX: &str = ".partial";

/// The file the device settings are kept in.
const SETTINGS_FILE: &str = "settings.json";
/// The file the Master Key is kept in, under the Passphrase (spec: KD-9).
const MASTER_KEY_FILE: &str = "master-key.cfmk";
/// The file a Library's previous per-Library OAuth grant was kept in, sealed
/// under its Master Key's token-cache purpose key (spec: KD-10). Read only to
/// promote it into an account's cache (spec: SA-8).
const PREVIOUS_TOKEN_CACHE_FILE: &str = "token-cache.cftc";
/// The file the Library's account-cache key envelope is kept in: the key of the
/// account it references, wrapped under this Library's own purpose key
/// (spec: SA-9, KD-12).
const ACCOUNT_ENVELOPE_FILE: &str = "account.cfke";
/// The file the catalog is kept in.
const INDEX_FILE: &str = "index.sqlite";
/// The file the running server's key is kept in, so that a caller on this
/// device can read it and nobody else's account can.
const SERVER_KEY_FILE: &str = "server-key";
/// The file a running server holds its lock on, so that one server at a time
/// serves this Library (spec: LA-8).
const SERVER_LOCK_FILE: &str = "server.lock";
/// The directory encrypted Containers wait in until they are uploaded.
const SPOOL_DIRECTORY: &str = "spool";
/// The directory the parcels a fetch read are kept in (spec: PK-21).
const PARCEL_DIRECTORY: &str = "parcels";
/// The directory a Library an earlier build put here has its records replayed
/// into afresh, to read its name from, while it is first opened.
const NAMING_DIRECTORY: &str = "naming";

/// One Library's directory on this device, and the seven things in it.
///
/// A Drive Library's grant is not among them: it belongs to the account the
/// Library references and is kept once, under the device's accounts. What is
/// here is the envelope that opens it, which is the Library's alone
/// (spec: SA-8, SA-9).
///
/// Everything a device keeps for a Library is under one directory named after
/// the Library's ID (spec: FM-18), so removing the directory removes the
/// device's whole part in that Library. Nothing a person chose names a
/// directory: the Library's name is anything at all (spec: FM-15), and is kept
/// as a copy in the settings file rather than spelled into the layout. That is
/// also why the settings file carries no paths: the layout says where each
/// piece is, and a file that also said would be a second answer able to
/// disagree with the first.
///
/// A Library an earlier build put here is in a directory named after what this
/// device called it then. It is found there as well as under its ID, and the
/// first open by this build moves it to its ID (see
/// [`open_library`](crate::open_library) and
/// [`move_to_its_id`](crate::move_to_its_id), which a server moves it with
/// before it holds it).
///
/// The directory is not required to exist: `of` works out where a
/// Library of a given ID goes, which is what putting one on this device needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryDir {
    /// What a message says this Library is: its name where this device knows
    /// one, and its ID otherwise.
    name: String,
    path: PathBuf,
}

impl LibraryDir {
    /// Where the Library with the ID `library_id` is kept, whether or not it
    /// is on this device.
    pub(crate) fn of(library_id: LibraryId) -> Result<Self> {
        let id = library_id.to_hex();
        Ok(Self {
            path: libraries_root()?.join(&id),
            name: id,
        })
    }

    /// A directory beside the Libraries, called `component`: where a Library
    /// is staged before its ID is known.
    pub(crate) fn beside(component: &str) -> Result<Self> {
        Ok(Self {
            path: libraries_root()?.join(component),
            name: component.to_owned(),
        })
    }

    /// The same directory, called `name` in what is said about it.
    pub(crate) fn called(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// What this device calls the Library in what it says about it: its name
    /// where one is known, and its ID otherwise.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The directory everything about this Library is kept in.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The directory this one is built in before it is finished: its own
    /// name with the staging suffix after it.
    ///
    /// The same accessors work on it, so every step of a creation writes
    /// through the staging directory and the last step is a rename.
    pub fn staging(&self) -> Self {
        let mut staged = self.path.clone().into_os_string();
        staged.push(STAGING_SUFFIX);
        Self {
            name: self.name.clone(),
            path: PathBuf::from(staged),
        }
    }

    /// The name of the directory an earlier build kept this Library in, where
    /// it is in one: a directory named after anything but `library_id`.
    ///
    /// That name was what the device called the Library then, and it is the
    /// name a Library whose catalog holds none is given when it is first
    /// opened by this build (spec: FM-15).
    pub(crate) fn earlier_name(&self, library_id: LibraryId) -> Option<&str> {
        let name = self.path.file_name()?.to_str()?;
        (name != library_id.to_hex()).then_some(name)
    }

    /// The device settings file, which is the contract this crate and the
    /// explorer both read.
    pub fn settings_file(&self) -> PathBuf {
        self.path.join(SETTINGS_FILE)
    }

    /// The Master Key as this device stores it, under the Passphrase.
    pub fn master_key_file(&self) -> PathBuf {
        self.path.join(MASTER_KEY_FILE)
    }

    /// The Library's previous per-Library grant, where a build before accounts
    /// left one (spec: KD-10).
    ///
    /// Nothing writes it any more; it is read once, to promote it into an
    /// account's cache, and removed afterwards (spec: SA-8).
    pub fn previous_token_cache_file(&self) -> PathBuf {
        self.path.join(PREVIOUS_TOKEN_CACHE_FILE)
    }

    /// The account-cache key envelope of the account this Library references,
    /// for a Library on a provider that needs a grant (spec: SA-9, KD-12).
    pub fn account_envelope_file(&self) -> PathBuf {
        self.path.join(ACCOUNT_ENVELOPE_FILE)
    }

    /// The catalog of this Library.
    pub fn index_file(&self) -> PathBuf {
        self.path.join(INDEX_FILE)
    }

    /// The key the server that is serving this Library admits its callers by.
    ///
    /// The one file here that no flow creates and no flow reads: it is written
    /// by a server as it starts and is meaningless the moment that process
    /// ends.
    pub fn server_key_file(&self) -> PathBuf {
        self.path.join(SERVER_KEY_FILE)
    }

    /// The file the server serving this Library holds its lock on (spec: LA-8).
    ///
    /// Beside the key rather than under a directory of running state, because it
    /// says the same kind of thing: that a server is up, and which one. It
    /// outlives the process that made it the way the key file does, and means as
    /// little afterwards — a lock nobody holds is what the next server takes.
    pub fn server_lock_file(&self) -> PathBuf {
        self.path.join(SERVER_LOCK_FILE)
    }

    /// Where encrypted Containers wait until they are uploaded.
    pub fn spool_dir(&self) -> PathBuf {
        self.path.join(SPOOL_DIRECTORY)
    }

    /// Where the parcels a fetch read are kept until they have served their
    /// purpose (spec: PK-21).
    ///
    /// Beside the spool and for the same reason: it is ciphertext this device
    /// wrote and will read back, and it belongs to this Library and to nothing
    /// in the folders the Library is mapped onto. Not made with the Library —
    /// the first parcel a fetch keeps makes it.
    pub fn parcel_dir(&self) -> PathBuf {
        self.path.join(PARCEL_DIRECTORY)
    }

    /// Where a Library an earlier build put here has its records replayed into
    /// afresh while it is first opened, to read the name they give it
    /// (spec: FM-15).
    ///
    /// A catalog of its own and not the Library's: it is made, read and
    /// removed within that one open, and one an interrupted open left is
    /// removed by the next.
    pub(crate) fn naming_dir(&self) -> PathBuf {
        self.path.join(NAMING_DIRECTORY)
    }

    /// Whether a whole Library is in this directory.
    ///
    /// The settings file is what is asked about rather than the directory: a
    /// directory with no settings in it is what an interrupted removal leaves,
    /// and it is not a Library anything can be opened from.
    pub fn is_present(&self) -> bool {
        self.settings_file().is_file()
    }
}

/// The names in the directory at `path`, sorted, or none where there is no
/// directory.
pub(crate) fn entries(path: &Path) -> Result<Vec<String>> {
    let listing = match fs::read_dir(path) {
        Ok(listing) => listing,
        Err(cause) if cause.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(cause) => return Err(LocalIoError::new(LocalOperation::Reading, path, cause).into()),
    };
    let mut names = Vec::new();
    for entry in listing {
        let entry = entry.map_err(Error::local(LocalOperation::Reading, path))?;
        // A name that is not Unicode is not one this build gave anything.
        if let Ok(name) = entry.file_name().into_string() {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

/// Coffret's own directory under the state directory the platform names, or
/// the one [`STATE_DIRECTORY`] names in its place: where the Libraries are
/// kept, under `libraries`, and where a shell keeps what it holds beside them.
pub fn state_directory() -> Result<PathBuf> {
    state_root()
}

/// The directory Libraries are kept under.
pub(crate) fn libraries_root() -> Result<PathBuf> {
    Ok(state_root()?.join("libraries"))
}

/// The directory the device keeps its accounts in, beside the Libraries
/// (spec: SA-8).
pub(crate) fn accounts_root() -> Result<PathBuf> {
    Ok(state_root()?.join("accounts"))
}

/// Coffret's own directory under the state directory the platform names.
///
/// An empty value is a variable exported from something that held nothing
/// rather than a request to keep Libraries at the root of the filesystem —
/// the reading `coffret-logging` gives the same variables.
fn state_root() -> Result<PathBuf> {
    #[cfg(test)]
    if let Some(root) = isolated::root() {
        return Ok(root);
    }
    if let Some(root) = env::var_os(STATE_DIRECTORY).filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(root));
    }
    let state = match env::var_os("XDG_STATE_HOME").filter(|value| !value.is_empty()) {
        Some(state) => PathBuf::from(state),
        None => {
            let home = env::var_os("HOME").filter(|value| !value.is_empty());
            PathBuf::from(home.ok_or(Error::NoStateDirectory)?).join(".local/state")
        }
    };
    Ok(state.join("coffret"))
}

/// A state directory one case has to itself.
///
/// Libraries are told apart by ID, so cases sharing one state directory stay
/// out of each other's way; accounts are the device's and not a Library's, so a
/// case that counts them, or removes the Libraries that reference one, needs a
/// device of its own. The override is the calling thread's, which is every
/// thread a `#[tokio::test]` runs its flow on.
#[cfg(test)]
pub(crate) mod isolated {
    use std::cell::RefCell;
    use std::path::PathBuf;

    thread_local! {
        static ROOT: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
    }

    /// The directory this thread's state is kept under, where a case set one.
    pub(super) fn root() -> Option<PathBuf> {
        ROOT.with(|root| root.borrow().clone())
    }

    /// Gives this thread a fresh state directory until the guard is dropped.
    pub(crate) fn device() -> Device {
        let directory = tempfile::tempdir().expect("a temporary directory must be available");
        ROOT.with(|root| *root.borrow_mut() = Some(directory.path().to_path_buf()));
        Device { directory }
    }

    /// One case's device, for as long as it is held.
    pub(crate) struct Device {
        directory: tempfile::TempDir,
    }

    impl Device {
        /// Where this device keeps everything.
        pub(crate) fn path(&self) -> &std::path::Path {
            self.directory.path()
        }
    }

    impl Drop for Device {
        fn drop(&mut self) {
            ROOT.with(|root| *root.borrow_mut() = None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ID a case's Library `seed` carries.
    pub(super) fn library_id(seed: u8) -> LibraryId {
        LibraryId::from_bytes([seed; LibraryId::BYTE_LEN])
    }

    // Everything a device keeps for a Library sits in the directory named after
    // its ID, which is what lets the settings file carry no paths at all.
    #[test]
    fn every_file_is_under_the_directory_named_after_the_library() {
        let dir = LibraryDir {
            name: "alpha".to_owned(),
            path: PathBuf::from("/state/coffret/libraries/alpha"),
        };

        assert_eq!(
            dir.settings_file(),
            Path::new("/state/coffret/libraries/alpha/settings.json")
        );
        assert_eq!(
            dir.master_key_file(),
            Path::new("/state/coffret/libraries/alpha/master-key.cfmk")
        );
        assert_eq!(
            dir.previous_token_cache_file(),
            Path::new("/state/coffret/libraries/alpha/token-cache.cftc")
        );
        assert_eq!(
            dir.account_envelope_file(),
            Path::new("/state/coffret/libraries/alpha/account.cfke")
        );
        assert_eq!(
            dir.index_file(),
            Path::new("/state/coffret/libraries/alpha/index.sqlite")
        );
        assert_eq!(
            dir.server_key_file(),
            Path::new("/state/coffret/libraries/alpha/server-key")
        );
        assert_eq!(
            dir.server_lock_file(),
            Path::new("/state/coffret/libraries/alpha/server.lock")
        );
        assert_eq!(
            dir.spool_dir(),
            Path::new("/state/coffret/libraries/alpha/spool")
        );
        assert_eq!(
            dir.parcel_dir(),
            Path::new("/state/coffret/libraries/alpha/parcels")
        );
    }

    // A Library's directory is its ID, whatever it is called (spec: FM-18).
    #[test]
    fn a_library_is_kept_under_its_id() {
        let device = isolated::device();
        let dir = LibraryDir::of(library_id(7)).unwrap();
        assert_eq!(
            dir.path(),
            device.path().join("libraries").join(library_id(7).to_hex())
        );
        assert_eq!(dir.earlier_name(library_id(7)), None);
    }

    // A creation writes through the staging directory and renames at the end,
    // so the staging directory is a sibling and takes the same accessors.
    #[test]
    fn a_staging_directory_is_a_sibling_of_the_finished_one() {
        let dir = LibraryDir {
            name: "alpha".to_owned(),
            path: PathBuf::from("/state/coffret/libraries/0123456789abcdef"),
        };
        let staging = dir.staging();

        assert_eq!(
            staging.path(),
            Path::new("/state/coffret/libraries/0123456789abcdef.partial")
        );
        assert_eq!(
            staging.settings_file(),
            Path::new("/state/coffret/libraries/0123456789abcdef.partial/settings.json")
        );
        assert_eq!(staging.name(), "alpha");
    }
}
