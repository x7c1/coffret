use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use coffret_usecase::commit::CommitError;
use coffret_usecase::fetch::FetchError;
use coffret_usecase::freeze::FreezeError;
use coffret_usecase::sync::SyncError;

use crate::library_dir::STAGING_SUFFIX;

/// Result alias for this crate's own fallible surface.
pub type Result<T> = std::result::Result<T, Error>;

/// What can go wrong keeping a Library on this device.
///
/// Every variant is a state a person can be told about and act on — the name is
/// not a directory name, the Library is already here, the Passphrase was not
/// the one this file was written under, the grant has run out — rather than a
/// rendering of some lower layer's message. What a lower layer reported travels
/// as the typed `cause` it reported, so a caller printing the chain sees the
/// format crate's, the Index's, or the gateway's own answer rather than a copy
/// of it made here.
#[derive(Debug)]
pub enum Error {
    /// The name given for a Library is not one a directory can be called.
    InvalidLibraryName {
        /// The name that was asked for.
        name: String,
        /// What is wrong with it.
        defect: NameDefect,
    },
    /// There is nowhere to keep Libraries: neither the state directory nor a
    /// home directory is named in the environment.
    NoStateDirectory,
    /// A Library of this name is already on this device.
    ///
    /// Creating one is refused rather than merged into: the directory holds a
    /// Master Key and a catalog, and a second `init` over them would draw a
    /// second Master Key and a second Library ID, stranding everything the
    /// first one named on Storage (spec: FM-18).
    LibraryExists {
        /// The Library that is already here.
        name: String,
        /// The directory it occupies.
        path: PathBuf,
    },
    /// No Library of this name is on this device.
    NoSuchLibrary {
        /// The Library that was asked for.
        name: String,
        /// Where one of that name would be.
        path: PathBuf,
    },
    /// A file or directory under the Library could not be read or written.
    Local {
        /// What was being done to it.
        doing: &'static str,
        /// The file or directory it was being done to.
        path: PathBuf,
        /// What the operating system reported.
        cause: io::Error,
    },
    /// The settings file holds something this build cannot read.
    MalformedSettings {
        /// The file that was read.
        path: PathBuf,
        /// What the reader reported.
        cause: serde_json::Error,
    },
    /// The settings file is in a shape this build has no reading for.
    ///
    /// Reported rather than repaired, and reported rather than replaced: the
    /// file names the Library's place on Storage, and a build that guessed at a
    /// shape it does not know could point a device at the wrong Library.
    UnsupportedSettingsVersion {
        /// The file that was read.
        path: PathBuf,
        /// The version it carries.
        version: u32,
        /// The version this build writes and reads.
        expected: u32,
    },
    /// The settings could not be encoded, so none of them were written.
    UnencodableSettings {
        /// The file they were meant for.
        path: PathBuf,
        /// What the encoder reported.
        cause: serde_json::Error,
    },
    /// The stored Master Key file could not be opened.
    ///
    /// The Passphrase not being the one the file was written under is the
    /// ordinary case, and it arrives as the format crate's own
    /// `AuthenticationFailed`: the bytes authenticate as a whole or not at all,
    /// so nothing in the file is read as key material on the way to finding out
    /// (spec: DK-5, KD-9).
    MasterKeyNotUnlocked {
        /// The file that was read.
        path: PathBuf,
        /// What the format layer reported.
        cause: coffret_format::Error,
    },
    /// The key material a new Library is built from could not be produced.
    ///
    /// The entropy source refused, or the Passphrase could not be derived from.
    /// Either way nothing was drawn and nothing was written.
    KeyMaterial {
        /// What the format layer reported.
        cause: coffret_format::Error,
    },
    /// The key a server would admit its callers by could not be drawn.
    ///
    /// The entropy source refused. Nothing was written and no port was bound: a
    /// key anything could guess is not a weaker boundary than the real one, it
    /// is no boundary at all.
    ServerKeyNotDrawn {
        /// What the entropy source reported.
        detail: String,
    },
    /// The Library ID could not be placed under the prefix that was asked for.
    MalformedStoragePrefix {
        /// What the model layer reported.
        cause: coffret_model::Error,
    },
    /// The catalog could not be opened, read, or written.
    Index {
        /// What the Index reported.
        cause: coffret_usecase::IndexError,
    },
    /// A step that reaches Google Drive did not complete.
    Drive {
        /// What the gateway reported.
        cause: google_drive_store::Error,
    },
    /// The Library is not on Google Drive, so there is no grant to renew.
    NotADriveLibrary {
        /// The Library that was asked for.
        name: String,
    },
    /// The Drive Library has no usable grant, so nothing can reach its Storage.
    ///
    /// A cache that is absent and a cache that will not open are one verdict
    /// here and are still told apart in `cause`: an unreadable token cache is
    /// never read as "nothing is cached" (spec: KD-10).
    NotAuthorized {
        /// The Library whose grant is missing.
        name: String,
        /// Why there is no grant, where a file was there and could not be read.
        cause: Option<google_drive_store::Error>,
    },
    /// The prefix a mapping was to be recorded under is not one top-level
    /// component of the Library.
    ///
    /// A mapping is keyed by the Library root or by exactly one top-level
    /// component, so a prefix with a separator in it names a subtree no mapping
    /// can stand for (spec: EP-9).
    MalformedMappingPrefix {
        /// The prefix that was asked for.
        prefix: String,
        /// What is wrong with it.
        defect: NameDefect,
    },
    /// The local root a mapping was to be recorded against is not a directory
    /// on this device.
    ///
    /// A root that is unmounted is a state a scan reports at the time it looks
    /// (spec: EP-12); a root that has never existed is a typo, and recording it
    /// would turn every later scan into that report.
    NoSuchLocalRoot {
        /// The root that was asked for.
        path: PathBuf,
        /// What the operating system reported, where it reported anything.
        cause: Option<io::Error>,
    },
    /// Whoever was asked for the Passphrase did not give one.
    ///
    /// The Passphrase reaches this crate through a callback the caller supplies,
    /// so that every refusal needing no key is made before a person is asked for
    /// one. What the callback reported travels whole: it is the terminal's, or
    /// the explorer's, and this layer has nothing to add to it.
    PassphraseNotGiven {
        /// What the caller that was asked reported.
        cause: Box<dyn error::Error + Send + Sync>,
    },
    /// The bucket a Library was to live in is not one this device can use.
    ///
    /// Asked before a Library is created, because on S3 nothing else would ask
    /// until the first sync: a prefix exists by being written under, so a
    /// mistyped bucket, an endpoint nothing is listening at, and credentials the
    /// SDK could not resolve all look exactly like a Library that has never been
    /// synced (spec: FM-18).
    ///
    /// The variant is the verdict — no Library goes here — and the cause is
    /// which of those it was, classified by the gateway rather than left as a
    /// message to read: a bucket S3 answered about and does not hold arrives as
    /// `NotFound`, credentials as `Unauthenticated` or `PermissionDenied`, and
    /// an endpoint nothing is listening at as `Transport`.
    BucketUnreachable {
        /// The bucket that was asked about.
        bucket: String,
        /// Why this device cannot put a Library there, in the Storage port's
        /// own words.
        cause: coffret_usecase::Error,
    },
    /// The Recovery Code that was entered is not one.
    ///
    /// It arrives as the format crate's own refusal, which names the check the
    /// string failed — a mistyped character, a checksum that does not hold, a
    /// version this build does not know — and releases no key material either
    /// way (spec: KD-11).
    MalformedRecoveryCode {
        /// What the format layer reported.
        cause: coffret_format::Error,
    },
    /// The place given for an existing Library does not name one.
    ///
    /// A Library's objects live under `coffret-<library id>` — a folder of that
    /// name on Drive, a key prefix ending in it on S3 — and the Library ID is
    /// read back out of it (spec: FM-18). Somewhere else may hold anything at
    /// all; what it does not hold is this Library.
    NotALibraryFolder {
        /// What was given: the folder's name on Drive, the prefix on S3.
        location: String,
        /// Why the ID in it is not one, where the name had the right shape and
        /// the ID did not.
        cause: Option<coffret_model::Error>,
    },
    /// A sync did not finish.
    Sync {
        /// What the flow reported.
        cause: SyncError,
    },
    /// A freeze did not finish.
    Freeze {
        /// What the flow reported.
        cause: FreezeError,
    },
    /// A fetch did not finish.
    Fetch {
        /// What the flow reported.
        cause: FetchError,
    },
    /// The catalog was not brought to the Library's head.
    ///
    /// It fails in the commit flow's vocabulary because it *is* that flow's
    /// routine (spec: CK-9): Storage did not answer, or what it answered with is
    /// not control state this device can replay. Nothing was written to the
    /// Library, because a catch-up commits nothing — but the catalog may well
    /// have moved part of the way, a replay being one record at a time and each
    /// one carrying the checkpoint to the head it became (spec: CP-1, CK-1).
    /// Wherever it stopped is a state the Library really was in and the next run
    /// starts from there, which is what lets a reader go on browsing either way.
    CatchUp {
        /// What the flow reported.
        cause: CommitError,
    },
    /// The Library was not created, and nothing of it was left on this device.
    ///
    /// The staging directory the steps ran in is removed before this is
    /// reported, so a second attempt starts from nothing. `orphan_folder` is
    /// the one thing a failure can leave behind that this crate cannot take
    /// back: a folder created on Drive before a later step failed.
    ///
    /// It is `None` where no folder was created *and* where the create is what
    /// failed, which are not the same state and cannot be told apart from
    /// here: a folder create is not idempotent and Drive mints the id, so an
    /// answer lost on the way back leaves a folder whose id never arrived. So
    /// a failure at [`CreationStep::AppFolder`] says to look before creating
    /// the Library again rather than claiming nothing is there.
    LibraryNotCreated {
        /// The Library that was being created.
        name: String,
        /// The step that failed.
        step: CreationStep,
        /// The app folder left on Drive, where one was created first.
        orphan_folder: Option<String>,
        /// What that step reported.
        cause: Box<Error>,
    },
    /// The Library was not joined, and nothing of it was left on this device.
    ///
    /// The staging directory the steps ran in is removed before this is
    /// reported, so a second attempt starts from nothing. Nothing can be left
    /// behind on Storage either, which is what makes this the simpler half of
    /// [`LibraryNotCreated`](Self::LibraryNotCreated): joining creates nothing
    /// there — the app folder is already the Library's, and the first commit
    /// after the join is what puts anything new in it.
    LibraryNotJoined {
        /// The Library that was being joined.
        name: String,
        /// The step that failed.
        step: CreationStep,
        /// What that step reported.
        cause: Box<Error>,
    },
}

/// What is wrong with a name that has to be one path component.
///
/// A Library's name is the name of its directory and a mapping's prefix is one
/// top-level component of the Library, so both are held to the same shape and
/// refused in the same vocabulary.
#[derive(Debug)]
pub enum NameDefect {
    /// Nothing was given.
    Empty,
    /// It holds a path separator, so it names more than one component.
    Separator,
    /// It is `.` or `..`, which name a directory rather than sit in one.
    Relative,
    /// It holds a control character, which no name should carry.
    Control,
    /// It ends in the suffix a Library being created is staged under, so it
    /// would collide with another Library's half-built directory.
    StagingSuffix,
}

impl fmt::Display for NameDefect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => f.write_str("it is empty"),
            Self::Separator => f.write_str("it holds a path separator"),
            Self::Relative => f.write_str("it names a directory rather than sits in one"),
            Self::Control => f.write_str("it holds a control character"),
            // The suffix is named rather than described: a person told their
            // name "ends in the suffix a Library is staged under" has been told
            // which name is refused but not which ending to drop.
            Self::StagingSuffix => write!(
                f,
                "it ends in {STAGING_SUFFIX:?}, which is what a Library being created is staged \
                 under"
            ),
        }
    }
}

/// Which step of creating or joining a Library a failure happened at.
///
/// What went wrong is in the cause; this says what was being attempted, which
/// is what tells a Passphrase that could not be stored apart from a grant that
/// was never given and from a catalog that could not be made.
///
/// The two flows share it because they are the same sequence over a Library
/// that does not exist yet and one that does: only the app-folder step differs,
/// and it differs in direction — one flow creates the folder, the other reads
/// back the name of one that is already there.
///
/// Every step here is one a staging directory is open for, which is why drawing
/// the Library ID and asking the bucket whether it is there are not among them:
/// both are settled before anything is staged, and each answers with a failure
/// of its own — [`Error::KeyMaterial`] and [`Error::BucketUnreachable`] — rather
/// than with a Library that was not created.
#[derive(Debug)]
pub enum CreationStep {
    /// Writing the Master Key under the Passphrase.
    StoredMasterKey,
    /// Asking the person for a grant on the Storage provider.
    Authorization,
    /// Creating the Library's app folder (spec: FM-18).
    AppFolder,
    /// Reading the name of the app folder a Library was said to live in
    /// (spec: FM-18).
    AppFolderName,
    /// Creating the catalog.
    Index,
    /// Creating the spool the encrypted files wait to be uploaded from.
    Spool,
    /// Writing the settings file.
    Settings,
    /// Moving the finished directory into place.
    Publish,
}

impl fmt::Display for CreationStep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let said = match self {
            Self::StoredMasterKey => "storing the Master Key under the Passphrase",
            Self::Authorization => "asking for a grant on the Storage provider",
            Self::AppFolder => "creating the Library's app folder",
            Self::AppFolderName => "reading the name of the Library's app folder",
            Self::Index => "creating the catalog",
            Self::Spool => "creating the spool directory",
            Self::Settings => "writing the settings file",
            Self::Publish => "moving the finished Library directory into place",
        };
        f.write_str(said)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLibraryName { name, defect } => {
                write!(f, "{name:?} cannot name a Library: {defect}")
            }
            Self::NoStateDirectory => f.write_str(
                "neither COFFRET_STATE_DIR, XDG_STATE_HOME, nor HOME is set, \
                 so there is nowhere to keep Libraries",
            ),
            Self::LibraryExists { name, path } => {
                write!(f, "the Library {name:?} is already at {}", path.display())
            }
            Self::NoSuchLibrary { name, path } => write!(
                f,
                "no Library {name:?} is on this device; nothing is at {}",
                path.display()
            ),
            Self::Local { doing, path, .. } => {
                write!(f, "{doing} at {}", path.display())
            }
            Self::MalformedSettings { path, .. } => write!(
                f,
                "the settings at {} hold something this build cannot read",
                path.display()
            ),
            Self::UnsupportedSettingsVersion {
                path,
                version,
                expected,
            } => write!(
                f,
                "the settings at {} are version {version}, and this build reads version {expected}",
                path.display()
            ),
            Self::UnencodableSettings { path, .. } => write!(
                f,
                "the settings could not be encoded, so nothing was written to {}",
                path.display()
            ),
            Self::MasterKeyNotUnlocked { path, .. } => write!(
                f,
                "the Master Key at {} did not open; the Passphrase may not be the one it was \
                 written under",
                path.display()
            ),
            Self::KeyMaterial { .. } => {
                f.write_str("the key material a new Library is built from could not be produced")
            }
            Self::ServerKeyNotDrawn { detail } => write!(
                f,
                "the key this server would admit its callers by could not be drawn: {detail}"
            ),
            Self::MalformedStoragePrefix { .. } => {
                f.write_str("the Library has no place under the Storage prefix that was asked for")
            }
            Self::Index { .. } => f.write_str("the Library's catalog could not be used"),
            // "A step that reaches Drive" rather than "a call to Drive": what
            // this wraps includes the failure to build the HTTP client, which
            // reaches nothing. The head line has to stay true of every cause
            // printed under it.
            Self::Drive { .. } => f.write_str("a step that reaches Google Drive did not complete"),
            Self::NotADriveLibrary { name } => write!(
                f,
                "the Library {name:?} is not on Google Drive, so it has no grant to renew"
            ),
            Self::NotAuthorized { name, .. } => write!(
                f,
                "the Library {name:?} has no usable grant on Google Drive; \
                 run `coffret authorize --library {name}`"
            ),
            Self::MalformedMappingPrefix { prefix, defect } => write!(
                f,
                "{prefix:?} cannot be mapped: a mapping stands for one top-level component, \
                 and {defect}"
            ),
            Self::NoSuchLocalRoot { path, .. } => {
                write!(f, "{} is not a directory on this device", path.display())
            }
            Self::PassphraseNotGiven { .. } => f.write_str("no Passphrase was given"),
            // What went wrong is the cause's to say — the bucket may be absent,
            // the credentials refused, or the endpoint silent — and this says
            // only which bucket it was and that nothing came of it.
            Self::BucketUnreachable { bucket, .. } => write!(
                f,
                "the bucket {bucket:?} cannot hold a Library; nothing was created"
            ),
            Self::MalformedRecoveryCode { .. } => {
                f.write_str("what was entered is not a Recovery Code")
            }
            // Both halves of the rule, because one variant answers both flows
            // and the reader knows which one they are in: the folder name is
            // what Drive was asked about, and the trailing separator is the
            // likeliest way an S3 prefix ends up here — a prefix without it
            // satisfies everything the first half asks for, so a message that
            // stopped there would state a rule the person had already met.
            Self::NotALibraryFolder { location, .. } => write!(
                f,
                "{location:?} is not where a Library lives: a Library's own folder is named \
                 {:?} followed by sixteen hex characters, and on S3 its prefix is that name \
                 with a {:?} after it",
                coffret_model::LibraryId::APP_FOLDER_PREFIX,
                "/"
            ),
            Self::Sync { .. } => f.write_str("the sync did not finish"),
            Self::Freeze { .. } => f.write_str("the freeze did not finish"),
            Self::Fetch { .. } => f.write_str("the fetch did not finish"),
            Self::CatchUp { .. } => {
                f.write_str("the catalog was not brought to the Library's head")
            }
            Self::LibraryNotJoined { name, step, .. } => {
                write!(f, "the Library {name:?} was not joined: {step} failed")
            }
            Self::LibraryNotCreated {
                name,
                step,
                orphan_folder,
                ..
            } => {
                write!(f, "the Library {name:?} was not created: {step} failed")?;
                match orphan_folder {
                    Some(folder) => write!(
                        f,
                        "; the folder {folder:?} was created on Drive first and is still there"
                    ),
                    // The id is exactly what did not arrive, so where to look
                    // is the only thing left to say — and saying nothing would
                    // invite a second `init` that leaves two folders behind.
                    None if matches!(step, CreationStep::AppFolder) => f.write_str(
                        "; a folder may have been created before the answer was lost, so look \
                         for a `coffret-` folder on Drive before creating this Library again",
                    ),
                    None => Ok(()),
                }
            }
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::InvalidLibraryName { .. }
            | Self::NoStateDirectory
            | Self::LibraryExists { .. }
            | Self::NoSuchLibrary { .. }
            | Self::NotADriveLibrary { .. }
            | Self::MalformedMappingPrefix { .. }
            | Self::ServerKeyNotDrawn { .. }
            | Self::UnsupportedSettingsVersion { .. } => None,
            Self::Local { cause, .. } => Some(cause),
            Self::MalformedSettings { cause, .. } | Self::UnencodableSettings { cause, .. } => {
                Some(cause)
            }
            Self::MasterKeyNotUnlocked { cause, .. } | Self::KeyMaterial { cause } => Some(cause),
            Self::MalformedStoragePrefix { cause } => Some(cause),
            Self::Index { cause } => Some(cause),
            Self::Drive { cause } => Some(cause),
            Self::NotAuthorized { cause, .. } => cause
                .as_ref()
                .map(|cause| cause as &(dyn error::Error + 'static)),
            Self::NoSuchLocalRoot { cause, .. } => cause
                .as_ref()
                .map(|cause| cause as &(dyn error::Error + 'static)),
            Self::PassphraseNotGiven { cause } => Some(cause.as_ref()),
            Self::BucketUnreachable { cause, .. } => Some(cause),
            Self::MalformedRecoveryCode { cause } => Some(cause),
            Self::NotALibraryFolder { cause, .. } => cause
                .as_ref()
                .map(|cause| cause as &(dyn error::Error + 'static)),
            Self::Sync { cause } => Some(cause),
            Self::Freeze { cause } => Some(cause),
            Self::Fetch { cause } => Some(cause),
            Self::CatchUp { cause } => Some(cause),
            Self::LibraryNotCreated { cause, .. } | Self::LibraryNotJoined { cause, .. } => {
                Some(cause.as_ref())
            }
        }
    }
}

impl Error {
    /// Names a local file or directory that could not be read or written.
    pub(crate) fn local(
        doing: &'static str,
        path: impl Into<PathBuf>,
    ) -> impl FnOnce(io::Error) -> Self {
        let path = path.into();
        move |cause| Self::Local { doing, path, cause }
    }
}

impl From<coffret_usecase::IndexError> for Error {
    fn from(cause: coffret_usecase::IndexError) -> Self {
        Self::Index { cause }
    }
}

impl From<google_drive_store::Error> for Error {
    fn from(cause: google_drive_store::Error) -> Self {
        Self::Drive { cause }
    }
}

impl From<SyncError> for Error {
    fn from(cause: SyncError) -> Self {
        Self::Sync { cause }
    }
}

impl From<FreezeError> for Error {
    fn from(cause: FreezeError) -> Self {
        Self::Freeze { cause }
    }
}

impl From<FetchError> for Error {
    fn from(cause: FetchError) -> Self {
        Self::Fetch { cause }
    }
}

impl From<CommitError> for Error {
    /// The one flow that reports the commit's vocabulary on its own is the
    /// catalog catch-up: every other caller of it is a sync or a fetch, and both
    /// wrap it in their own refusal before it reaches here.
    fn from(cause: CommitError) -> Self {
        Self::CatchUp { cause }
    }
}
