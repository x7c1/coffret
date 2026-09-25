use std::env;
use std::path::{Path, PathBuf};

use crate::error::{Error, NameDefect, Result};

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

/// One Library's directory on this device, and the seven things in it.
///
/// A Drive Library's grant is not among them: it belongs to the account the
/// Library references and is kept once, under the device's accounts. What is
/// here is the envelope that opens it, which is the Library's alone
/// (spec: SA-8, SA-9).
///
/// Everything a device keeps for a Library is under one directory named after
/// the Library, so nothing but the directory's own name has to be configured
/// and removing the directory removes the device's whole part in that Library.
/// That is why the settings file carries no paths: the layout says where each
/// piece is, and a file that also said would be a second answer able to
/// disagree with the first.
///
/// The directory is not required to exist. `resolve` works out where a Library
/// of a given name would be, which is what both the call that creates one and
/// the call that refuses to create a second one need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryDir {
    name: String,
    path: PathBuf,
}

impl LibraryDir {
    /// Works out where the Library called `name` is kept.
    ///
    /// The name is one path component and is held to that here rather than
    /// wherever a path is next joined: it comes from a person, it becomes a
    /// directory name, and a name with a separator in it would put a Library
    /// somewhere no other command would look for it.
    ///
    /// One check more than a path component owes, and only here: a name ending
    /// in the staging suffix would name the directory another Library of that
    /// name is half-built in. That is this directory's own concern, so nothing
    /// else that has to be one component is held to it.
    pub fn resolve(name: &str) -> Result<Self> {
        let staging_collision = || {
            name.ends_with(STAGING_SUFFIX)
                .then_some(NameDefect::StagingSuffix)
        };
        if let Some(defect) = defect_in(name).or_else(staging_collision) {
            return Err(Error::InvalidLibraryName {
                name: name.to_owned(),
                defect,
            });
        }
        Ok(Self {
            name: name.to_owned(),
            path: libraries_root()?.join(name),
        })
    }

    /// The Library's name on this device.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The directory everything about this Library is kept in.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The directory a Library of this name is built in before it is finished.
    ///
    /// The same accessors work on it, so every step of a creation writes
    /// through the staging directory and the last step is a rename.
    pub fn staging(&self) -> Self {
        Self {
            name: self.name.clone(),
            path: self
                .path
                .with_file_name(format!("{}{STAGING_SUFFIX}", self.name)),
        }
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

    /// Whether a whole Library of this name is on this device.
    ///
    /// The settings file is what is asked about rather than the directory: a
    /// directory with no settings in it is what an interrupted removal leaves,
    /// and it is not a Library anything can be opened from.
    pub fn is_present(&self) -> bool {
        self.settings_file().is_file()
    }
}

/// What is wrong with `name` as the name of a directory on this device, if
/// anything is.
///
/// One path component, and one this device's own naming leaves alone: a name
/// with a separator in it would put a Library where no other command would look
/// for it, and a name carrying a control character is one a person cannot see
/// what they typed of. It is not an Entry Path and is held to none of EP-2's
/// shape — a Library's name never reaches Storage and names nothing inside the
/// Library.
///
/// The staging suffix is asked about by [`LibraryDir::resolve`] and not here,
/// because it is the one rule about where a name would land rather than about
/// the name.
fn defect_in(name: &str) -> Option<NameDefect> {
    if name.is_empty() {
        return Some(NameDefect::Empty);
    }
    if name == "." || name == ".." {
        return Some(NameDefect::Relative);
    }
    if name.contains('/') || name.contains('\\') {
        return Some(NameDefect::Separator);
    }
    if name.chars().any(char::is_control) {
        return Some(NameDefect::Control);
    }
    None
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
/// Libraries are told apart by name, so cases sharing one state directory stay
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

    // The name becomes a directory name, so a name that is not one component is
    // refused before anything is looked at, let alone created.
    #[test]
    fn a_name_that_is_not_one_path_component_is_refused() {
        assert!(matches!(refusal(""), NameDefect::Empty));
        assert!(matches!(refusal(".."), NameDefect::Relative));
        assert!(matches!(refusal("."), NameDefect::Relative));
        assert!(matches!(refusal("albums/2026"), NameDefect::Separator));
        assert!(matches!(refusal("../escape"), NameDefect::Separator));
        assert!(matches!(refusal("albums\\2026"), NameDefect::Separator));
        assert!(matches!(refusal("albums\n2026"), NameDefect::Control));
        assert!(matches!(
            refusal("albums.partial"),
            NameDefect::StagingSuffix
        ));
    }

    /// What `resolve` refused `name` for, or a panic saying what it did instead.
    fn refusal(name: &str) -> NameDefect {
        match LibraryDir::resolve(name) {
            Err(Error::InvalidLibraryName { defect, .. }) => defect,
            other => panic!("{name:?} must be refused as a name, got {other:?}"),
        }
    }

    // The staging suffix is this directory's own concern. A Library folder
    // called `albums.partial` is an ordinary top-level component (spec: EP-9),
    // and refusing to map it would be this device's private naming leaking into
    // what a person may keep in their Library.
    #[test]
    fn only_a_library_name_is_held_to_the_staging_suffix() {
        assert!(defect_in("albums.partial").is_none());
        assert!(matches!(
            LibraryDir::resolve("albums.partial"),
            Err(Error::InvalidLibraryName {
                defect: NameDefect::StagingSuffix,
                ..
            })
        ));
    }

    // Everything a device keeps for a Library sits in the directory named after
    // it, which is what lets the settings file carry no paths at all.
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
    }

    // A creation writes through the staging directory and renames at the end,
    // so the staging directory is a sibling and takes the same accessors.
    #[test]
    fn a_staging_directory_is_a_sibling_of_the_finished_one() {
        let dir = LibraryDir {
            name: "alpha".to_owned(),
            path: PathBuf::from("/state/coffret/libraries/alpha"),
        };
        let staging = dir.staging();

        assert_eq!(
            staging.path(),
            Path::new("/state/coffret/libraries/alpha.partial")
        );
        assert_eq!(
            staging.settings_file(),
            Path::new("/state/coffret/libraries/alpha.partial/settings.json")
        );
        assert_eq!(staging.name(), "alpha");
    }
}
