use std::error;
use std::fmt;
use std::io;
use std::path::PathBuf;

use coffret_model::{EntryPath, Redacted};
use coffret_usecase::commit::CommitError;
use coffret_usecase::fetch::{BelowRootError, DescentError, FetchError};
use coffret_usecase::freeze::FreezeError;
use coffret_usecase::root_marker::{MalformedMarker, MANAGEMENT_AREA, MARKER_FILE};
use coffret_usecase::sync::SyncError;
use coffret_usecase::{LocalIoError, LocalOperation, RefusedRoot, RootRefused};

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
    /// A file or directory on this device could not be read or written: one of
    /// the Library's own, or one in a folder it maps.
    ///
    /// It is the use case layer's own [`LocalIoError`], carried rather than
    /// restated: this device's disk answers in one vocabulary whichever crate
    /// touched it — the operation, the path, and what the operating system
    /// said — so a caller asking which operation refused matches on
    /// [`LocalOperation`] here exactly as it would behind a capability.
    Local(LocalIoError),
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
        ///
        /// The value and not a rendering of it, so that a caller can read the
        /// kind the source named and the cause chain reaches it.
        cause: getrandom::Error,
    },
    /// A server is already serving this Library on this device (spec: LA-8).
    ///
    /// One server at a time serves a Library, so a second start is refused
    /// before the Passphrase is asked for and before anything of the first
    /// server's is touched: it keeps its key, its callers, and its hold on the
    /// Library. What refused is an exclusive lock the running server holds, so
    /// the answer is the operating system's rather than a file this crate
    /// reads and believes.
    LibraryAlreadyServed {
        /// The Library that is already being served.
        name: String,
        /// The process serving it, where it wrote its number down.
        ///
        /// `None` where it had not got that far or the number could not be
        /// read. It is a courtesy and never the verdict: what says a server is
        /// there is the lock it holds, and the number only says which one to
        /// stop.
        by: Option<u32>,
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
        ///
        /// Boxed so that the gateway's enum does not set this one's width: a
        /// field added over there would otherwise widen every `Result` this
        /// crate returns, and the pointer keeps the two crates' sizes apart
        /// while the typed cause still travels.
        cause: Box<google_drive_store::Error>,
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
        ///
        /// Boxed for the reason `Drive`'s cause is.
        cause: Option<Box<google_drive_store::Error>>,
    },
    /// The prefix a mapping was to be recorded under is not one top-level
    /// component of the Library.
    ///
    /// A mapping is keyed by the Library root or by exactly one top-level
    /// component (spec: EP-9), and there are two ways to miss that. Either the
    /// text is no Entry Path at all, which the model says in its own words and
    /// which travels here as the cause; or it is one and names more than one
    /// component, which is this crate's own rule and has no cause below it.
    MalformedMappingPrefix {
        /// The prefix that was asked for.
        prefix: String,
        /// Why it is no Entry Path, where that is what it is not.
        ///
        /// `None` where it is one and names a subtree rather than a top-level
        /// component.
        cause: Option<coffret_model::Error>,
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
    /// Something that is not a directory of coffret's own stands at the name a
    /// mapped root's management area is reserved under (spec: EP-13, EP-14).
    ///
    /// A symbolic link and an ordinary file are one state here: what the person
    /// has to do about either is the same, and following the link to find out
    /// which it was is exactly what a descent below a mapped root may not do
    /// (spec: EP-8).
    ///
    /// What the open reached may be spelled `.COFFRET`, which is the whole of
    /// why the sentence does not claim the exact name is what is standing
    /// there: on a case-folding volume the open by the reserved name reaches a
    /// file or a link of the person's under any spelling of it, and the handle
    /// it failed on carries none (spec: EP-14). The spelling is not read back
    /// the way the missing marker's is, because here it changes neither the
    /// verdict nor the gesture — something that is not a folder is standing at
    /// the reserved name whichever of the spellings it wears, and moving it is
    /// what settles it either way.
    ManagementAreaNotADirectory {
        /// The root whose management area it is.
        root: PathBuf,
    },
    /// A mapped root's management area is there and holds no marker
    /// (spec: EP-13).
    ///
    /// What an interrupted registration leaves. It is refused rather than
    /// completed, because a marker written into a management area somebody else
    /// made would give the root an identity that run never agreed to.
    ///
    /// Only where the folder the descent reached is spelled exactly
    /// `.coffret`. A case-folding volume can put the descent inside a folder of
    /// the person's own, and that is
    /// [`ManagementAreaFolded`](Self::ManagementAreaFolded) instead.
    ManagementAreaIncomplete {
        /// The root whose management area it is.
        root: PathBuf,
    },
    /// What the descent into a mapped root's management area reached is a
    /// folder whose name only folds to the reserved one (spec: EP-14).
    ///
    /// An open by name hands back a file descriptor, and a descriptor carries
    /// no name, so a registration on a case-folding volume descends `.coffret`
    /// and may land in a folder somebody called `.COFFRET` for their own
    /// reasons. Finding no marker in it,
    /// [`ManagementAreaIncomplete`](Self::ManagementAreaIncomplete) would tell
    /// them an interrupted registration left it and to get it out of the way —
    /// which is a sentence about coffret's folder said about theirs. So the
    /// spelling is read back off the parent directory where the refusal is
    /// composed, which is the only place it is still available, and this is
    /// what a folder of theirs gets instead.
    ManagementAreaFolded {
        /// The root whose registration met it.
        root: PathBuf,
        /// The name standing there, as the directory spells it.
        name: String,
    },
    /// The marker in a mapped root's management area is not a regular file
    /// (spec: EP-13).
    ///
    /// A symbolic link, a folder, a device, a pipe: one refusal, for the reason
    /// [`ManagementAreaNotADirectory`](Self::ManagementAreaNotADirectory) is
    /// one.
    MarkerNotARegularFile {
        /// The root whose marker it is.
        root: PathBuf,
    },
    /// The marker in a mapped root names no identity (spec: EP-13).
    ///
    /// Its content is not the sixteen characters an identity is spelled in, or
    /// it runs on past the cap a marker is read to. Refused and not rewritten,
    /// with a new identity asked for or without: replacing it would take an
    /// identity away from whichever device wrote what is standing there.
    MarkerMalformed {
        /// The root whose marker it is.
        root: PathBuf,
        /// What is wrong with the content.
        cause: MalformedMarker,
    },
    /// The mapped root a file was to be placed into is not the root the mapping
    /// was recorded against (spec: EP-13).
    ///
    /// Raised where this device is placing files it was handed — an upload the
    /// browser dropped in, a write already under way — and the request fails
    /// as a whole, the way a declined placement fails one (spec: EP-11). An
    /// upload may hand several, and they all go through this one root, so a
    /// refusal of it leaves none of them anywhere to go. A folder fetch meets
    /// the same refusal and reports the mapping instead, carrying on with the
    /// device's other mappings.
    ///
    /// Nothing was written and nothing was repaired. Only recording the mapping
    /// ever writes or adopts a marker, so what gets a folder out of this is
    /// recording it again — which is what the message says, naming the mapping
    /// it is about: a device with more than one would otherwise be told to
    /// record "the mapping" with nothing to point the gesture at.
    ///
    /// Carried as the [`RefusedRoot`] the use-case layer already reports a
    /// mapping by, the way
    /// [`FetchError::RefusedRoot`](coffret_usecase::fetch::FetchError::RefusedRoot)
    /// carries it: the value is made where the refusal is learned, and one set of
    /// fields keeps one refusal one shape whichever reading met it.
    RootRefused(RefusedRoot),
    /// Whether the mapped root a file was to be placed into is the root the
    /// mapping was recorded against could not be asked at all (spec: EP-13).
    ///
    /// The operating system refused the read that settles it — a permission the
    /// process has not on the root's own management area is the ordinary shape
    /// of it — so nothing is known about the mapping either way. Deliberately
    /// *not* a [`RootRefused`](Self::RootRefused): a folder nobody could read
    /// the marker of has not been found to be the wrong folder, and telling a
    /// person to record the mapping again would send them to fix something that
    /// is not broken. Nothing was written and nothing was repaired, the way
    /// nothing is on any reading of a marker.
    ///
    /// Its own variant rather than a [`Local`](Self::Local) because of how far
    /// it reaches. The marker stands in the root itself, so an answer that did
    /// not come is settled for every file going through that root before the
    /// first of them is written: a caller handed several at once, as one
    /// upload's files are, has nothing left to place through this mapping and
    /// stops there rather than meeting the same refusal once per file
    /// (spec: EP-11). What the operating system said is carried whole for the
    /// same reason [`Local`](Self::Local) carries it.
    RootUnvouched {
        /// The mapped root nothing could be learned about, for the message that
        /// names it.
        local_root: PathBuf,
        /// What the operating system said, and which of the root's own names it
        /// said it about.
        cause: LocalIoError,
    },
    /// The identity a mapped root was to carry could not be drawn
    /// (spec: EP-13).
    ///
    /// The entropy source refused. Nothing was written and no mapping was
    /// recorded: an identity anything could guess would certify nothing, and one
    /// two roots could share would certify the wrong thing.
    RootMarkerNotDrawn {
        /// The root it was to be drawn for.
        root: PathBuf,
        /// What the format layer reported.
        cause: coffret_format::Error,
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
    /// Whoever was asked for the Recovery Code did not give one.
    ///
    /// The code reaches this crate through a callback so local name and
    /// provider-location refusals happen before a terminal or pipe is read.
    RecoveryCodeNotGiven {
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
    /// `NotFound` of the configured location, credentials as `Unauthenticated`
    /// or `PermissionDenied`, and an endpoint nothing is listening at as
    /// `Transport`.
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
    /// Where on this device a file belongs was not settled.
    ///
    /// Its own variant rather than a [`Fetch`](Self::Fetch), because nothing was
    /// fetched and nothing was going to be. The question is EP-9's alone — which
    /// folder of this device stands for a part of the Library, and what the
    /// Entry Path becomes inside it — and it is asked by a caller with no
    /// transfer in hand at all: something reporting where a file would go,
    /// rather than whether one is there or what is standing where it would be.
    /// A caller handed "the fetch did not finish" over an
    /// unmapped path would be reading a sentence about a transfer that was
    /// never begun, and would have to open the chain to find that the answer is
    /// about a mapping.
    ///
    /// The question asked and not the file's fate, which is why a drop that is
    /// turned away is [`FileNotTakenIn`](Self::FileNotTakenIn) instead: nothing
    /// was being written here, so there is nothing this could say about a file
    /// beyond where it would have stood. And why a read that could not say what
    /// is standing in a mapped folder is
    /// [`LocalFilesNotRead`](Self::LocalFilesNotRead): that one answers with the
    /// files rather than with a path, so where they belong is not the question
    /// it left unanswered. And why a reader that went to open the file itself is
    /// [`LocalFileNotOpened`](Self::LocalFileNotOpened), for the same reason
    /// once more: it was owed the file and not the path to it.
    ///
    /// What was wrong with the path is the `cause`'s to say, and it says it in
    /// the fetch's vocabulary because the translation is the fetch's own
    /// (spec: EP-9): there is one implementation of that rule and this is the
    /// door onto it.
    LocalPathNotSettled {
        /// What the translation reported.
        cause: FetchError,
    },
    /// A file somebody handed this device was not taken in.
    ///
    /// Its own variant rather than a [`Fetch`](Self::Fetch) for the reason
    /// [`LocalPathNotSettled`](Self::LocalPathNotSettled) is one — nothing is
    /// fetched on the way in, the bytes being already here — and beside that
    /// one rather than folded into it. Not every refusal carried here is the
    /// EP-9 translation's verdict: a component coffret keeps for itself is
    /// refused before a mapping is read at all (spec: EP-11, EP-14), and
    /// "where the file belongs was not settled" would report a lookup that fell
    /// short where what happened is that the name is not one this device will
    /// hold a file under.
    ///
    /// So the sentence is about the file rather than about the path, which
    /// keeps it true of every refusal that reaches here — including a catalog
    /// that could not be read and therefore decided nothing about the path at
    /// all. Which refusal it was is the `cause`'s to say, in the fetch's
    /// vocabulary because where a file may stand on this device is written once
    /// and the flow that places files is where (spec: EP-4, EP-9).
    FileNotTakenIn {
        /// What the write into the mapped folder reported.
        cause: FetchError,
    },
    /// What this device has of its own there was not read.
    ///
    /// The third gesture that speaks the fetch's vocabulary without fetching,
    /// beside the two above it: somebody looking at what is in a folder of
    /// theirs that the Library does not hold. Nothing is transferred to answer
    /// that — the files are already standing in the mapped folder, which is the
    /// whole reason the folder is read rather than the catalog asked
    /// (spec: EP-10). A person who opened a folder and was handed "the fetch
    /// did not finish" would be reading about a transfer nobody began.
    ///
    /// Not [`FileNotTakenIn`](Self::FileNotTakenIn), which a read may not
    /// borrow: no file was handed over here, so there is none for "was not
    /// taken in" to be about, and the sentence would have somebody hunting for
    /// an upload they never made. Not
    /// [`LocalPathNotSettled`](Self::LocalPathNotSettled) either, because what
    /// reaches here is not always that translation's verdict: a component that
    /// folds to the management area's name is refused before a mapping is read
    /// at all (spec: EP-14), on the grounds that a case-folding volume leaves
    /// the read no way to tell a folder of the person's from this device's own.
    ///
    /// So the sentence is about the answer that did not come back, which is
    /// what the reader was owed and is true of every refusal carried here —
    /// including a catalog that could not be read and decided nothing about the
    /// folder at all. Which refusal it was is the `cause`'s to say, in the
    /// fetch's vocabulary because which folder of this device stands for a part
    /// of the Library is written once (spec: EP-9).
    LocalFilesNotRead {
        /// What the read of the mapped folder reported.
        cause: FetchError,
    },
    /// What this device has for an Entry was not opened.
    ///
    /// The fourth gesture that speaks the fetch's vocabulary without fetching,
    /// and the one most people meet: somebody opening a file the Library *does*
    /// hold an Entry for, out of the folder this device places it in. Nothing is
    /// transferred to answer that — the reader is asking because a
    /// materialization record already says the file is standing there
    /// (spec: EP-10) — so "the fetch did not finish" is the one sentence that
    /// can never be true of it.
    ///
    /// Not [`LocalFilesNotRead`](Self::LocalFilesNotRead), whose sentence is
    /// about what this device has *of its own*: that is the gesture over a
    /// folder the Library holds nothing in, and here the Library holds the Entry
    /// and the file is this device's copy of it, so that sentence would point a
    /// reader at the wrong thing to go and look at. Not
    /// [`LocalPathNotSettled`](Self::LocalPathNotSettled) either, although every
    /// refusal carried here comes through the same EP-9 translation: that one
    /// answers with a path, and this answers with a file, so a path that did
    /// settle is only half of what was owed. And not
    /// [`FileNotTakenIn`](Self::FileNotTakenIn), nothing having been handed over
    /// to take in.
    ///
    /// So the sentence is about what did not open, which is true of every
    /// refusal carried here — a path no mapping reaches, a path no file here can
    /// stand for, a catalog that could not be read, and a row that outlived the
    /// Entry it was written for, which is a state to go on from rather than one
    /// to fail at (spec: EP-10). Which of them it was is the `cause`'s to say,
    /// in the fetch's vocabulary because the translation it went through is the
    /// fetch's own (spec: EP-9).
    LocalFileNotOpened {
        /// What the translation reported.
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

/// What is wrong with the name a Library has on this device.
///
/// The name becomes a directory name beside the other Libraries', so it is one
/// path component as this device spells one. It is not an Entry Path and shares
/// no vocabulary with one: a backslash and a control character are refused here
/// and carried without comment inside the Library, because a name here is what a
/// person navigates their own disk by.
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
    /// Asking the place a Library was said to live in whether it holds what a
    /// Library keeps at the top of its own place (spec: FM-12).
    ///
    /// Only where that question needs a grant, which is Drive: on S3 it is
    /// asked before anything is staged and answers with
    /// [`Error::BucketUnreachable`], as the bucket check beside it does.
    LibraryObject,
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
            Self::LibraryObject => "asking whether the Library's app folder holds anything of it",
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
            // Which file it was and what was being done to it, in the
            // operation's own word. What the operating system answered is not
            // repeated here: it is the `io::Error` underneath, and a shell that
            // shows the chain prints it there — printing it inside this line as
            // well would say the whole refusal twice. The file is named because
            // this line is read by the person standing at the device with the
            // Library in front of them. Keeping a path out of a diagnostic
            // event is `redacted`'s job, not this one's (spec: EL-1).
            Self::Local(refused) => write!(
                f,
                "{} could not be {}",
                refused.path.display(),
                refused.operation
            ),
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
            // What the entropy source said is the value underneath, which a
            // shell showing the chain prints there; saying it inside this line
            // as well would spell one refusal twice over.
            Self::ServerKeyNotDrawn { .. } => {
                f.write_str("the key this server would admit its callers by could not be drawn")
            }
            // The Library and the process, and nothing about the key, the file
            // it is in, or the file the lock is on. Which file says a server is
            // running is this crate's own arrangement, and a person told to go
            // and delete one would be told to do the one thing that does not
            // help: the lock is the operating system's and is gone the moment
            // the process is (spec: LA-4, LA-8). What ends this state is
            // stopping that server, so that is what the sentence says — after a
            // semicolon and in the same clause-per-line voice every other
            // refusal here is written in, so that a shell printing the chain
            // reads one continuous line rather than a sentence of its own.
            Self::LibraryAlreadyServed { name, by } => {
                write!(
                    f,
                    "the Library {name:?} is already being served on this device"
                )?;
                if let Some(by) = by {
                    write!(f, ", by process {by}")?;
                }
                f.write_str(
                    "; one server at a time serves a Library, so stop that one before starting \
                     another",
                )
            }
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
            // The model's refusal quotes the prefix and says which part of the
            // shape went, and it is printed under this line rather than inside
            // it — a shell that shows the chain would otherwise say the whole
            // refusal twice.
            Self::MalformedMappingPrefix {
                prefix,
                cause: Some(_),
            } => write!(f, "{prefix:?} cannot be mapped"),
            Self::MalformedMappingPrefix {
                prefix,
                cause: None,
            } => write!(
                f,
                "{prefix:?} cannot be mapped: a mapping stands for one top-level component of \
                 the Library, and this names more than one"
            ),
            Self::NoSuchLocalRoot { path, .. } => {
                write!(f, "{} is not a directory on this device", path.display())
            }
            // Each of these says which folder it is about and what is standing
            // where, because that is the whole of what the person has to go and
            // look at — and each says nothing was recorded, since a mapping
            // half-recorded against a root with no identity is exactly what
            // none of them leaves behind. Where the thing a person would reach
            // for next is the run that just refused, the arm rules that out as
            // well: recording the mapping again, with a new identity asked for
            // or without, is not what gets a folder out of these states, and a
            // message that left it unsaid would have them spend a run finding
            // that out.
            // The name coffret keeps rather than the spelling on disk: a
            // case-folding volume may have handed the open a `.COFFRET` of the
            // person's, and a sentence saying `.coffret` is standing there would
            // send them looking for a name their folder does not hold
            // (spec: EP-14). Which spelling it wears changes nothing they do.
            Self::ManagementAreaNotADirectory { root } => write!(
                f,
                "the name coffret keeps for its own folder in {} — {MANAGEMENT_AREA}, or a \
                 spelling differing from it only in case — has something standing at it that is \
                 not a folder of coffret's; nothing was written and nothing was recorded",
                root.display()
            ),
            Self::ManagementAreaIncomplete { root } => write!(
                f,
                "{} holds a {MANAGEMENT_AREA} folder with no {MARKER_FILE} in it, which is what \
                 an interrupted registration leaves; nothing was written and nothing was \
                 recorded, and recording the mapping again meets this same refusal until that \
                 folder is out of the way",
                root.display()
            ),
            // The one of these about a folder that is the person's rather than
            // coffret's, so it is the one that names the folder to them — which
            // is the whole of what it has to offer, since coffret cannot tell
            // them which of the two the volume handed it. It says nothing about
            // an interrupted registration, because none of theirs was
            // interrupted, and it asks for a rename rather than for the folder
            // to be got out of the way.
            Self::ManagementAreaFolded { root, name } => write!(
                f,
                "{} holds a folder named {name}, which this filesystem does not tell apart from \
                 {MANAGEMENT_AREA}, the name coffret keeps for its own folder in a mapped root; \
                 nothing was written and nothing was recorded, and recording the mapping again \
                 meets this same refusal until that folder is renamed",
                root.display()
            ),
            Self::MarkerNotARegularFile { root } => write!(
                f,
                "{MANAGEMENT_AREA}/{MARKER_FILE} in {} is not a regular file; nothing was \
                 written and nothing was recorded, and a new identity asked for replaces one \
                 rather than repairing this",
                root.display()
            ),
            // What is wrong with the content is the reading's own answer and
            // travels as the cause, which a shell showing the chain prints
            // under this line rather than inside it as well.
            Self::MarkerMalformed { root, .. } => write!(
                f,
                "{MANAGEMENT_AREA}/{MARKER_FILE} in {} names no identity; nothing was \
                 written and nothing was recorded, and a new identity asked for replaces one \
                 rather than repairing this",
                root.display()
            ),
            // The one of these the marker's *reader* raises rather than its
            // writer, so it says what the others say with the tense changed:
            // nothing was placed, and recording that mapping again is the
            // gesture. Those words are the refusal's own rather than this
            // layer's, because a fetch that met the same state while placing the
            // rest of the Library owes a person the same ones, and a sentence
            // spelled out in both places is a sentence that can drift in one.
            Self::RootRefused(refusal) => write!(f, "{refusal}"),
            // The folder is named and the mapping is not sent for: nothing was
            // learned about it, so the one thing a person can act on is the
            // folder the read was refused in. Which of coffret's own names in it
            // refused comes from the refusal rather than being written out here,
            // because the marker is not the only one of them a reading passes
            // through: the folder holding it is refused on its own account, and
            // a sentence that named the marker for that would send a person to a
            // file whose own mode is perfectly sound. What the operating system
            // answered is the `io::Error` underneath, which a shell printing the
            // chain shows.
            Self::RootUnvouched { local_root, cause } => write!(
                f,
                "{} could not be checked against the mapping it was recorded for: {} in it \
                 could not be {}, and nothing was placed",
                local_root.display(),
                cause
                    .path
                    .strip_prefix(local_root)
                    .unwrap_or(&cause.path)
                    .display(),
                cause.operation
            ),
            // "No marker" rather than "nothing": the management area is made
            // before the identity that goes into it is drawn, so this is the one
            // of the five that may leave a folder of coffret's own behind — and
            // a person who reads that nothing happened and then meets
            // [`ManagementAreaIncomplete`](Self::ManagementAreaIncomplete) on
            // the next run has been told two things that cannot both be true.
            Self::RootMarkerNotDrawn { root, .. } => write!(
                f,
                "an identity for {} could not be drawn; no marker was written and nothing was \
                 recorded",
                root.display()
            ),
            Self::PassphraseNotGiven { .. } => f.write_str("no Passphrase was given"),
            Self::RecoveryCodeNotGiven { .. } => f.write_str("no Recovery Code was given"),
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
            // What this device would do with the file is not in it, because
            // nothing here was going to do anything with one: the question was
            // where the file belongs, and the answer is that there is not one.
            Self::LocalPathNotSettled { .. } => {
                f.write_str("where on this device that file belongs was not settled")
            }
            // Where it would have gone is not in it, because for some of these
            // there is nowhere it could have gone and for one of them nothing
            // was worked out at all. What a person handed over is the file, so
            // what did not happen to the file is the answer.
            Self::FileNotTakenIn { .. } => f.write_str("the file was not taken in"),
            // Which folder is not in it, the caller having just named one, and
            // neither is what was going to be done with what is in it: nothing
            // was, beyond showing it to whoever asked.
            Self::LocalFilesNotRead { .. } => {
                f.write_str("what this device has of its own there was not read")
            }
            // Which Entry is not in it, the caller having just named one, and
            // neither is what the bytes were wanted for: nothing here was going
            // to do anything with them but hand them over.
            Self::LocalFileNotOpened { .. } => {
                f.write_str("what this device has for that Entry was not opened")
            }
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
            | Self::LibraryAlreadyServed { .. }
            | Self::ManagementAreaNotADirectory { .. }
            | Self::ManagementAreaIncomplete { .. }
            | Self::ManagementAreaFolded { .. }
            | Self::MarkerNotARegularFile { .. }
            | Self::UnsupportedSettingsVersion { .. } => None,
            Self::MarkerMalformed { cause, .. } => Some(cause),
            // The marker's own refusal where that is what made it, so a printed
            // chain ends at what the file held rather than at the root.
            Self::RootRefused(refusal) => match &refusal.reason {
                RootRefused::MarkerMalformed { cause } => Some(cause),
                _ => None,
            },
            Self::RootMarkerNotDrawn { cause, .. } => Some(cause),
            Self::ServerKeyNotDrawn { cause } => Some(cause),
            Self::Local(refused) | Self::RootUnvouched { cause: refused, .. } => {
                Some(&refused.cause)
            }
            Self::MalformedSettings { cause, .. } | Self::UnencodableSettings { cause, .. } => {
                Some(cause)
            }
            Self::MasterKeyNotUnlocked { cause, .. } | Self::KeyMaterial { cause } => Some(cause),
            Self::MalformedStoragePrefix { cause } => Some(cause),
            // The model's refusal where the prefix is no Entry Path, and nothing
            // underneath where it is one and names more than one component.
            Self::MalformedMappingPrefix { cause, .. } => cause
                .as_ref()
                .map(|cause| cause as &(dyn error::Error + 'static)),
            Self::Index { cause } => Some(cause),
            Self::Drive { cause } => Some(cause.as_ref()),
            Self::NotAuthorized { cause, .. } => cause
                .as_ref()
                .map(|cause| cause.as_ref() as &(dyn error::Error + 'static)),
            Self::NoSuchLocalRoot { cause, .. } => cause
                .as_ref()
                .map(|cause| cause as &(dyn error::Error + 'static)),
            Self::PassphraseNotGiven { cause } | Self::RecoveryCodeNotGiven { cause } => {
                Some(cause.as_ref())
            }
            Self::BucketUnreachable { cause, .. } => Some(cause),
            Self::MalformedRecoveryCode { cause } => Some(cause),
            Self::NotALibraryFolder { cause, .. } => cause
                .as_ref()
                .map(|cause| cause as &(dyn error::Error + 'static)),
            Self::Sync { cause } => Some(cause),
            Self::Freeze { cause } => Some(cause),
            Self::Fetch { cause }
            | Self::LocalPathNotSettled { cause }
            | Self::FileNotTakenIn { cause }
            | Self::LocalFilesNotRead { cause }
            | Self::LocalFileNotOpened { cause } => Some(cause),
            Self::CatchUp { cause } => Some(cause),
            Self::LibraryNotCreated { cause, .. } | Self::LibraryNotJoined { cause, .. } => {
                Some(cause.as_ref())
            }
        }
    }
}

impl Redacted for Error {
    /// Which state of this device it is, and none of the names it is about.
    ///
    /// Almost every variant here is *identified* by something a person chose —
    /// the Library's name, the directory it occupies, the settings file, the
    /// bucket, the mapping prefix, the folder on Drive — because that is what
    /// makes the message useful to the one reader it is written for, who is
    /// standing at this device with the Library in front of them. None of it
    /// may be written down, so what a diagnostic event gets is the variant,
    /// the step-shaped facts around it, and the redacted cause underneath.
    ///
    /// Four causes deliberately stop here rather than going underneath.
    /// [`Drive`](Self::Drive) and [`NotAuthorized`](Self::NotAuthorized) carry
    /// the Drive gateway's own failure, which records itself where it happens
    /// with the bodies and URLs already redacted — and which, in the token
    /// cache's case, names a file on this device.
    /// [`PassphraseNotGiven`](Self::PassphraseNotGiven) and
    /// [`RecoveryCodeNotGiven`](Self::RecoveryCodeNotGiven) carry whatever the
    /// terminal or the explorer reported, which are boxed errors this layer
    /// knows nothing about. In all four the identity is what the log is for.
    ///
    /// [`ServerKeyNotDrawn`](Self::ServerKeyNotDrawn) goes the other way and
    /// writes its cause down as it stands, for the reason
    /// `coffret_format::Error` writes the same value down: what `getrandom`
    /// prints is about this machine's random source and names no path, no
    /// filename and nothing anybody chose (spec: EL-3, EL-4). Since the
    /// Library this key was for may not go with it, that is the whole of what a
    /// reader of this event can act on.
    fn redacted(&self) -> String {
        match self {
            Self::InvalidLibraryName { defect, .. } => {
                format!("Device::InvalidLibraryName(defect={defect})")
            }
            Self::NoStateDirectory => "Device::NoStateDirectory".to_owned(),
            Self::LibraryExists { .. } => "Device::LibraryExists".to_owned(),
            Self::NoSuchLibrary { .. } => "Device::NoSuchLibrary".to_owned(),
            Self::Local(refused) => format!("Device::Local: {}", refused.redacted()),
            Self::MalformedSettings { .. } => "Device::MalformedSettings".to_owned(),
            Self::UnsupportedSettingsVersion {
                version, expected, ..
            } => format!(
                "Device::UnsupportedSettingsVersion(version={version}, expected={expected})"
            ),
            Self::UnencodableSettings { .. } => "Device::UnencodableSettings".to_owned(),
            Self::MasterKeyNotUnlocked { cause, .. } => {
                format!("Device::MasterKeyNotUnlocked: {}", cause.redacted())
            }
            Self::KeyMaterial { cause } => {
                format!("Device::KeyMaterial: {}", cause.redacted())
            }
            Self::ServerKeyNotDrawn { cause } => format!("Device::ServerKeyNotDrawn: {cause}"),
            // The process number and not the Library's name. A process id is
            // the operating system's own and names nothing a person chose, so
            // it is evidence a diagnostic event may keep — and it is the one
            // fact worth keeping here, since what a reader of this event wants
            // to know is which two runs were racing (spec: EL-1).
            Self::LibraryAlreadyServed { by, .. } => format!(
                "Device::LibraryAlreadyServed(by={})",
                match by {
                    Some(by) => by.to_string(),
                    None => "unknown".to_owned(),
                }
            ),
            Self::MalformedStoragePrefix { cause } => {
                format!("Device::MalformedStoragePrefix: {}", cause.redacted())
            }
            Self::Index { cause } => format!("Device::Index: {}", cause.redacted()),
            Self::Drive { .. } => "Device::Drive".to_owned(),
            Self::NotADriveLibrary { .. } => "Device::NotADriveLibrary".to_owned(),
            Self::NotAuthorized { cause, .. } => format!(
                "Device::NotAuthorized(cached={})",
                match cause {
                    Some(_) => "unreadable",
                    None => "absent",
                }
            ),
            // The prefix is a folder somebody means to keep their files in, so
            // it is Library content and stays out of the diagnostic event;
            // what is left is which of the two rules it missed.
            Self::MalformedMappingPrefix { cause, .. } => match cause {
                Some(cause) => format!("Device::MalformedMappingPrefix: {}", cause.redacted()),
                None => "Device::MalformedMappingPrefix(more than one component)".to_owned(),
            },
            Self::NoSuchLocalRoot { cause, .. } => format!(
                "Device::NoSuchLocalRoot(kind={})",
                match cause {
                    Some(cause) => format!("{:?}", cause.kind()),
                    None => "none".to_owned(),
                }
            ),
            // The root is a folder somebody keeps their own files in, so it is
            // Library content and stays out of the event. Which state of the
            // management area it was does not name anything of theirs — it is
            // coffret's own vocabulary about coffret's own folder — so it stays.
            Self::ManagementAreaNotADirectory { .. } => {
                "Device::ManagementAreaNotADirectory".to_owned()
            }
            Self::ManagementAreaIncomplete { .. } => "Device::ManagementAreaIncomplete".to_owned(),
            // The root is the person's folder and so is the name standing in
            // it — this is the one of these refusals about a folder of theirs
            // rather than coffret's, so the name a person reads in the message
            // is exactly what an event must not carry (spec: EL-1).
            Self::ManagementAreaFolded { .. } => "Device::ManagementAreaFolded".to_owned(),
            Self::MarkerNotARegularFile { .. } => "Device::MarkerNotARegularFile".to_owned(),
            Self::MarkerMalformed { cause, .. } => {
                format!("Device::MarkerMalformed(defect={})", cause.defect())
            }
            // Which shape the wrong folder took, and neither the folder nor
            // either identity: the reason is coffret's own vocabulary about
            // coffret's own file (spec: EL-1).
            Self::RootRefused(refusal) => {
                format!("Device::RootRefused: {}", refusal.reason.redacted())
            }
            // Which operation the disk refused and what sort of refusal it was,
            // and neither the root nor the file under it: the folder is the
            // person's own name for it (spec: EL-1), and the identity here is
            // the variant, which is what tells a reader counting these that the
            // marker went unread rather than that a mapping is wrong.
            Self::RootUnvouched { cause, .. } => {
                format!("Device::RootUnvouched: {}", cause.redacted())
            }
            Self::RootMarkerNotDrawn { cause, .. } => {
                format!("Device::RootMarkerNotDrawn: {}", cause.redacted())
            }
            Self::PassphraseNotGiven { .. } => "Device::PassphraseNotGiven".to_owned(),
            Self::RecoveryCodeNotGiven { .. } => "Device::RecoveryCodeNotGiven".to_owned(),
            Self::BucketUnreachable { cause, .. } => {
                format!("Device::BucketUnreachable: {}", cause.redacted())
            }
            Self::MalformedRecoveryCode { cause } => {
                format!("Device::MalformedRecoveryCode: {}", cause.redacted())
            }
            Self::NotALibraryFolder { .. } => "Device::NotALibraryFolder".to_owned(),
            Self::Sync { cause } => format!("Device::Sync: {}", cause.redacted()),
            Self::Freeze { cause } => format!("Device::Freeze: {}", cause.redacted()),
            Self::Fetch { cause } => format!("Device::Fetch: {}", cause.redacted()),
            Self::LocalPathNotSettled { cause } => {
                format!("Device::LocalPathNotSettled: {}", cause.redacted())
            }
            Self::FileNotTakenIn { cause } => {
                format!("Device::FileNotTakenIn: {}", cause.redacted())
            }
            Self::LocalFilesNotRead { cause } => {
                format!("Device::LocalFilesNotRead: {}", cause.redacted())
            }
            Self::LocalFileNotOpened { cause } => {
                format!("Device::LocalFileNotOpened: {}", cause.redacted())
            }
            Self::CatchUp { cause } => format!("Device::CatchUp: {}", cause.redacted()),
            Self::LibraryNotCreated {
                step,
                orphan_folder,
                cause,
                ..
            } => format!(
                "Device::LibraryNotCreated(step={step:?}, orphan_folder={}): {}",
                orphan_folder.is_some(),
                cause.redacted(),
            ),
            Self::LibraryNotJoined { step, cause, .. } => format!(
                "Device::LibraryNotJoined(step={step:?}): {}",
                cause.redacted()
            ),
        }
    }
}

impl Error {
    /// Whether this is a code exchange the token endpoint refused, made by a
    /// client that sent no secret.
    ///
    /// The one refusal a shell has something of its own to add to: a client
    /// registered with a secret cannot be authorized without it, and where a
    /// secret would have come from — a variable, a flag, a settings file — is
    /// the shell's vocabulary rather than this crate's or the gateway's. So the
    /// fact travels up typed, and the shell that knows the name says it.
    ///
    /// The two creation failures are looked through because that is where this
    /// arrives from: an `init` or a `join` that got as far as the browser and
    /// no further reports the step, and the exchange's refusal is inside it.
    pub fn is_exchange_without_client_secret(&self) -> bool {
        match self {
            Self::Drive { cause } => {
                matches!(
                    cause.as_ref(),
                    google_drive_store::Error::CodeExchangeWithoutSecret { .. }
                )
            }
            Self::LibraryNotCreated { cause, .. } | Self::LibraryNotJoined { cause, .. } => {
                cause.is_exchange_without_client_secret()
            }
            _ => false,
        }
    }

    /// Names a local file or directory that could not be read or written.
    pub(crate) fn local(
        operation: LocalOperation,
        path: impl Into<PathBuf>,
    ) -> impl FnOnce(io::Error) -> Self {
        let path = path.into();
        move |cause| Self::Local(LocalIoError::new(operation, path, cause))
    }

    /// What a refused descent into a mapped folder means for one Entry Path.
    ///
    /// A fence the descent met — a component that is a symbolic link, or an
    /// ordinary file where a folder must be — is
    /// [`FetchError::UnmaterializablePath`], which is the same verdict the
    /// translation already gives a path no file on this device can stand for
    /// (spec: EP-2, EP-4, EP-11), inside
    /// [`FileNotTakenIn`](Self::FileNotTakenIn): the verdict is the fetch's and
    /// the gesture it answers is somebody handing a file over. The folder the
    /// descent stopped at travels with it: each file of an upload is one the
    /// person just handed over, and the one thing they can act on is which
    /// folder in the way is not a folder.
    ///
    /// A mapped root that will not vouch for itself is
    /// [`RootRefused`](Self::RootRefused), carrying the mapping, the folder, and
    /// which of EP-13's cases it was. The request fails as a whole, however
    /// many files this caller was handed: they all go through that one root, so
    /// there is no mapping to go on with — the reading EP-11 gives a single
    /// writer, and EP-13 repeats for a root whose identity is wrong.
    ///
    /// A mapped root that *could not be asked* is
    /// [`RootUnvouched`](Self::RootUnvouched), carrying the folder and what the
    /// operating system said. The request fails as a whole for the same reason
    /// and not for the same finding: the marker stands in the root every one of
    /// those files goes through, so a read of it that was refused is refused for
    /// all of them — and none of them is told the mapping is wrong, because
    /// nothing about the mapping was learned (spec: EP-11, EP-13).
    ///
    /// Everything else is the operating system's answer, which travels whole as
    /// the refusal the capability reported — the operation it was, the path it
    /// was on, and what the operating system said.
    ///
    /// `prefix` is the mapping the descent was made through, which the
    /// capability's own refusal cannot carry: a [`Destinations`] is handed the
    /// root and the components apart and knows nothing of Entry Paths, so the
    /// mapping is named back on this side of that call.
    ///
    /// [`Destinations`]: coffret_usecase::Destinations
    pub(crate) fn descent(
        refused: DescentError,
        prefix: Option<&EntryPath>,
        path: &EntryPath,
    ) -> Self {
        match refused {
            DescentError::Refused { root, reason } => Self::RootRefused(RefusedRoot {
                prefix: prefix.cloned(),
                local_root: root,
                reason,
            }),
            DescentError::Unvouched { root, cause } => Self::RootUnvouched {
                local_root: root,
                cause,
            },
            DescentError::Blocked { stopped_at } => {
                Self::below_root(BelowRootError::Blocked { stopped_at }, path)
            }
            DescentError::Io(refused) => Self::below_root(BelowRootError::Io(refused), path),
        }
    }

    /// The same, for a step taken below a root a descent has already vouched
    /// for.
    ///
    /// The two ways of [`descent`](Self::descent)'s four that are about the
    /// path, and the whole of what the calls an
    /// [`IncomingFile`](crate::IncomingFile) makes can report: the folder it
    /// writes through was opened by the descent
    /// [`receive_file`](crate::OpenLibrary::receive_file) made, and holding a
    /// root against the identity its mapping recorded is that descent's alone
    /// (spec: EP-13). So there is no mapping to name here, and no refusal about
    /// one to name it for.
    pub(crate) fn below_root(refused: BelowRootError, path: &EntryPath) -> Self {
        match refused {
            // Built rather than converted: `?` on this vocabulary means
            // `Error::Fetch`, and every caller of this one is on the way in
            // with somebody's file in hand.
            BelowRootError::Blocked { stopped_at } => Self::FileNotTakenIn {
                cause: FetchError::UnmaterializablePath {
                    path: path.clone(),
                    stopped_at: Some(stopped_at),
                },
            },
            BelowRootError::Io(refused) => Self::Local(refused),
        }
    }
}

impl From<LocalIoError> for Error {
    /// The same refusal in this crate's vocabulary, which is the use case's
    /// vocabulary for this device's disk: nothing is decided on the way, so
    /// `?` carries one into the other.
    fn from(refused: LocalIoError) -> Self {
        Self::Local(refused)
    }
}

impl From<coffret_usecase::IndexError> for Error {
    fn from(cause: coffret_usecase::IndexError) -> Self {
        Self::Index { cause }
    }
}

impl From<google_drive_store::Error> for Error {
    fn from(cause: google_drive_store::Error) -> Self {
        Self::Drive {
            cause: Box::new(cause),
        }
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
    /// A fetch is what raises this vocabulary nearly everywhere, so the `?` in a
    /// flow means [`Fetch`](Error::Fetch). The callers that raise it without
    /// fetching anything say so outright rather than leaning on this: the EP-9
    /// translation asked on its own is
    /// [`LocalPathNotSettled`](Error::LocalPathNotSettled), a file turned
    /// away on its way into a mapped folder is
    /// [`FileNotTakenIn`](Error::FileNotTakenIn), a read of what somebody has
    /// put in a mapped folder is
    /// [`LocalFilesNotRead`](Error::LocalFilesNotRead), and the opening of the
    /// file this device placed for an Entry is
    /// [`LocalFileNotOpened`](Error::LocalFileNotOpened).
    ///
    /// Those four are every caller in this crate that speaks this vocabulary
    /// without fetching, so what is left for `?` to carry is the flows that do
    /// fetch — where the outer sentence is the true one. A `?` on this
    /// vocabulary anywhere else is a caller that has not yet said which gesture
    /// it is refusing, rather than the shape to copy.
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

#[cfg(test)]
mod tests {
    use coffret_usecase::fetch::FetchError;

    use super::*;
    use crate::testing::entry_path;

    /// The links a caller printing `{error:#}` reads, outermost first.
    fn chain(error: &dyn error::Error) -> Vec<String> {
        let mut links = vec![error.to_string()];
        let mut below = error.source();
        while let Some(link) = below {
            links.push(link.to_string());
            below = link.source();
        }
        links
    }

    // The message names the Library and the directory it is in, which is what
    // the person standing at this device needs; the diagnostic event names
    // the state and nothing they called anything.
    #[test]
    fn a_library_that_is_already_here_is_recorded_without_its_name() {
        let error = Error::LibraryExists {
            name: "holiday-photos".to_owned(),
            path: PathBuf::from("/home/someone/.local/state/coffret/holiday-photos"),
        };

        assert!(error.to_string().contains("holiday-photos"));
        assert_eq!(error.redacted(), "Device::LibraryExists");
    }

    // LA-8 as EL-1 sees it. The person starting a second server is told which
    // Library of theirs it is about and which process to stop; the diagnostic
    // event keeps the process and none of the name.
    #[test]
    fn a_library_already_being_served_is_recorded_without_its_name() {
        let error = Error::LibraryAlreadyServed {
            name: "holiday-photos".to_owned(),
            by: Some(4213),
        };

        let said = error.to_string();
        assert!(said.contains("holiday-photos"), "{said}");
        assert!(said.contains("4213"), "{said}");
        assert_eq!(error.redacted(), "Device::LibraryAlreadyServed(by=4213)");

        // A server killed between taking the lock and writing its number down
        // leaves the sentence one clause shorter rather than no refusal at all:
        // what says a server is there is the lock, not the number.
        let anonymous = Error::LibraryAlreadyServed {
            name: "holiday-photos".to_owned(),
            by: None,
        };

        let said = anonymous.to_string();
        assert!(said.contains("holiday-photos"), "{said}");
        assert!(!said.contains("process"), "{said}");
        assert_eq!(
            anonymous.redacted(),
            "Device::LibraryAlreadyServed(by=unknown)"
        );
    }

    // The entropy source's refusal travels as the value it reported, so a
    // caller can read the kind it named off the chain. That kind is one of the
    // few things this vocabulary may write down, since it is about this
    // machine's random source and nothing anybody chose.
    #[test]
    fn a_server_key_that_could_not_be_drawn_carries_what_the_source_reported() {
        use std::error::Error as _;

        let reported = getrandom::Error::UNSUPPORTED;
        let error = Error::ServerKeyNotDrawn { cause: reported };

        let source = error.source().expect("the chain reaches the source");
        assert!(
            source.downcast_ref::<getrandom::Error>().is_some(),
            "the source is the value getrandom reported and not a rendering of it",
        );
        // Said once: this line names what could not be drawn, and what the
        // source reported is the link under it.
        assert_eq!(
            chain(&error),
            vec![
                "the key this server would admit its callers by could not be drawn".to_owned(),
                reported.to_string(),
            ],
        );
        // Composed from the source's own rendering rather than written out: a
        // reworded upstream sentence is not this layer's rendering changing. A
        // diagnostic event has no chain to walk, so this is the one rendering
        // that still spells the cause out.
        assert_eq!(
            error.redacted(),
            format!("Device::ServerKeyNotDrawn: {reported}"),
        );
    }

    // The chain a refusal reaches the log as, whole: which flow, which refusal
    // inside it, and the shape of the refusal — and no path from either end.
    #[test]
    fn a_fetch_that_could_not_place_a_file_records_the_whole_chain() {
        let error = Error::Fetch {
            cause: FetchError::UnmaterializablePath {
                path: entry_path("albums/spring.jpg"),
                stopped_at: Some(PathBuf::from("/home/someone/albums")),
            },
        };

        assert_eq!(
            error.redacted(),
            "Device::Fetch: Fetch::UnmaterializablePath(path_len=17, descent=blocked)",
        );
    }

    // A refusal about where a file belongs is not a refusal about a fetch, and
    // the chain says which it is at its own outermost link: nothing was
    // transferred, so a first sentence about a transfer would send whoever reads
    // it looking for one.
    #[test]
    fn a_path_that_could_not_be_placed_says_so_without_naming_a_fetch() {
        let error = Error::LocalPathNotSettled {
            cause: FetchError::UnmappedEntryPath {
                path: entry_path("albums/spring.jpg"),
            },
        };

        assert_eq!(
            chain(&error),
            vec![
                "where on this device that file belongs was not settled".to_owned(),
                FetchError::UnmappedEntryPath {
                    path: entry_path("albums/spring.jpg"),
                }
                .to_string(),
            ],
        );
        assert_eq!(
            error.redacted(),
            "Device::LocalPathNotSettled: Fetch::UnmappedEntryPath(path_len=17)",
        );
    }

    // The other half of the same rule, for the gesture that has a file in hand:
    // a drop that was turned away is answered as a drop. The vocabulary inside
    // the chain is the fetch's because the rule about where a file may stand on
    // this device is written once, and the sentence on the outside is about the
    // file, which is what the person did.
    #[test]
    fn a_file_that_was_not_taken_in_says_so_without_naming_a_fetch() {
        let error = Error::FileNotTakenIn {
            cause: FetchError::UnmappedEntryPath {
                path: entry_path("albums/spring.jpg"),
            },
        };

        assert_eq!(
            chain(&error),
            vec![
                "the file was not taken in".to_owned(),
                FetchError::UnmappedEntryPath {
                    path: entry_path("albums/spring.jpg"),
                }
                .to_string(),
            ],
        );
        assert_eq!(
            error.redacted(),
            "Device::FileNotTakenIn: Fetch::UnmappedEntryPath(path_len=17)",
        );
    }

    // And the third gesture, which has neither a transfer nor a file in hand:
    // somebody looking at what is in a folder of their own. The refusal kept
    // for a name a case-folding volume will not tell apart from this device's
    // management area is the one that reaches a reader, and it is refused
    // before any mapping is read — so the outer sentence is about the answer
    // that did not come back rather than about a path that was not settled.
    #[test]
    fn a_read_of_a_mapped_folder_says_what_was_not_read_without_naming_a_fetch() {
        let error = Error::LocalFilesNotRead {
            cause: FetchError::FoldedReservedComponent {
                path: entry_path("albums/.COFFRET"),
                component: ".COFFRET".to_owned(),
            },
        };

        assert_eq!(
            chain(&error),
            vec![
                "what this device has of its own there was not read".to_owned(),
                FetchError::FoldedReservedComponent {
                    path: entry_path("albums/.COFFRET"),
                    component: ".COFFRET".to_owned(),
                }
                .to_string(),
            ],
        );
        assert_eq!(
            error.redacted(),
            "Device::LocalFilesNotRead: Fetch::FoldedReservedComponent(path_len=15)",
        );
    }

    // And the fourth, which is the one a browser walks over every time somebody
    // opens a picture: the file this device placed for an Entry the Library
    // holds. The refusal chosen here is the one that would read worst under the
    // other sentences — the Library no longer holds the Entry, so nothing was
    // unsettled about a path and nothing of this device's own was being looked
    // at — and it is a state the caller goes on from rather than fails at
    // (spec: EP-10).
    #[test]
    fn a_placed_file_that_did_not_open_says_so_without_naming_a_fetch() {
        let error = Error::LocalFileNotOpened {
            cause: FetchError::EntryNotCurrent {
                path: entry_path("albums/spring.jpg"),
            },
        };

        assert_eq!(
            chain(&error),
            vec![
                "what this device has for that Entry was not opened".to_owned(),
                FetchError::EntryNotCurrent {
                    path: entry_path("albums/spring.jpg"),
                }
                .to_string(),
            ],
        );
        assert_eq!(
            error.redacted(),
            "Device::LocalFileNotOpened: Fetch::EntryNotCurrent(path_len=17)",
        );
    }

    // EL-1: the person standing at the device is told which file refused, since
    // that is the one thing they can go and look at; the diagnostic event
    // carries the operation and the kind of refusal and no part of the path.
    // The refusal travels whole, so its own `io::Error` is still the chain's
    // next link.
    #[test]
    fn a_local_refusal_names_the_file_for_a_person_and_never_for_the_log() {
        use std::error::Error as _;

        let error = Error::Local(LocalIoError::new(
            LocalOperation::Renaming,
            PathBuf::from("/home/someone/albums/spring.jpg"),
            io::Error::from(io::ErrorKind::PermissionDenied),
        ));

        assert!(
            error
                .to_string()
                .contains("/home/someone/albums/spring.jpg"),
            "{error}",
        );
        assert_eq!(
            error.redacted(),
            "Device::Local: Local::Io(operation=renamed, kind=PermissionDenied)",
        );

        // And the operating system's answer is said once, underneath, rather
        // than copied into the line above it as well.
        let answered = io::Error::from(io::ErrorKind::PermissionDenied).to_string();
        assert!(
            !error.to_string().contains(&answered),
            "the cause belongs under this line and not inside it: {error}",
        );
        assert_eq!(
            error.source().map(ToString::to_string),
            Some(answered),
            "the cause is still reachable",
        );
    }

    // EL-1, EL-2: every coffret error in a nested cause chain contributes its
    // log-safe rendering. The device-local Library name, the path derived from
    // it, and the foreign I/O message remain available to a person and absent
    // from the diagnostic form.
    #[test]
    fn a_nested_creation_failure_keeps_no_device_local_library_name() {
        const LIBRARY: &str = "Family Tax Records";
        const PRIVATE_PATH: &str =
            "/Users/alice/Library/Application Support/coffret/libraries/Family Tax Records.partial";
        let error = Error::LibraryNotCreated {
            name: LIBRARY.to_owned(),
            step: CreationStep::Publish,
            orphan_folder: None,
            cause: Box::new(Error::Local(LocalIoError::new(
                LocalOperation::Renaming,
                PRIVATE_PATH,
                io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!("rename of {PRIVATE_PATH} was denied"),
                ),
            ))),
        };

        let rendered = error.redacted();
        assert_eq!(
            rendered,
            "Device::LibraryNotCreated(step=Publish, orphan_folder=false): \
             Device::Local: Local::Io(operation=renamed, kind=PermissionDenied)"
        );
        assert!(!rendered.contains(LIBRARY), "{rendered}");
        assert!(!rendered.contains(PRIVATE_PATH), "{rendered}");
        assert!(error.to_string().contains(LIBRARY), "{error}");
    }

    // The one refusal a shell has words of its own for, as it actually
    // arrives: wrapped twice, the creation step over the Drive refusal. The
    // shell only ever holds the outer one, so the look-through is what decides
    // whether the variable gets named at all.
    #[test]
    fn a_creation_refused_at_the_exchange_without_a_secret_is_seen_through_its_wrappers() {
        let sent_no_secret = || Error::Drive {
            cause: Box::new(google_drive_store::Error::CodeExchangeWithoutSecret {
                status: 401,
                detail: r#"{"error":"invalid_client"}"#.to_owned(),
            }),
        };

        let created = Error::LibraryNotCreated {
            name: "holiday-photos".to_owned(),
            step: CreationStep::Authorization,
            orphan_folder: None,
            cause: Box::new(sent_no_secret()),
        };
        assert!(created.is_exchange_without_client_secret(), "{created}");

        let joined = Error::LibraryNotJoined {
            name: "holiday-photos".to_owned(),
            step: CreationStep::Authorization,
            cause: Box::new(sent_no_secret()),
        };
        assert!(joined.is_exchange_without_client_secret(), "{joined}");
    }

    // And the endpoint's own refusal of a client that did send its secret
    // answers no: that run was refused for some other reason, and naming the
    // variable would send somebody to change the one thing that was not the
    // matter.
    #[test]
    fn a_creation_refused_for_another_reason_says_nothing_about_a_secret() {
        let refused = Error::LibraryNotCreated {
            name: "holiday-photos".to_owned(),
            step: CreationStep::Authorization,
            orphan_folder: None,
            cause: Box::new(Error::Drive {
                cause: Box::new(google_drive_store::Error::TokenEndpoint {
                    status: 401,
                    detail: r#"{"error":"invalid_grant"}"#.to_owned(),
                }),
            }),
        };
        assert!(!refused.is_exchange_without_client_secret(), "{refused}");
    }

    // EP-2: a prefix that is no Entry Path is refused in the model's words, and
    // those words are printed once. The head line says what this layer knows —
    // which prefix, and that nothing was mapped — because a shell printing the
    // chain prints the cause under it and would otherwise say it twice.
    #[test]
    fn a_prefix_that_is_no_entry_path_says_the_reason_once() {
        use std::error::Error as _;

        let error = Error::MalformedMappingPrefix {
            prefix: "albums/".to_owned(),
            cause: Some(coffret_model::Error::MalformedEntryPath {
                path: "albums/".to_owned(),
                defect: coffret_model::PathDefect::TrailingSeparator,
            }),
        };

        assert_eq!(error.to_string(), "\"albums/\" cannot be mapped");
        assert_eq!(
            error.source().map(ToString::to_string).as_deref(),
            Some("\"albums/\" is not an Entry Path: it ends with a separator"),
        );
    }

    // EP-13: the marker's refusal reaches a person as one sentence per layer —
    // this crate's, which names the folder and the gesture; the reading's,
    // which says the content is no spelling of an identity; and the model's,
    // which says what that spelling would have had to be.
    #[test]
    fn a_malformed_marker_reaches_a_caller_as_one_sentence_per_layer() {
        const ROOT: &str = "/home/someone/Pictures/Holidays";
        let error = Error::MarkerMalformed {
            root: PathBuf::from(ROOT),
            cause: coffret_usecase::root_marker::parse(b"not an identity")
                .expect_err("that content names no identity"),
        };

        assert_eq!(
            chain(&error),
            vec![
                format!(
                    "{MANAGEMENT_AREA}/{MARKER_FILE} in {ROOT} names no identity; nothing was \
                     written and nothing was recorded, and a new identity asked for replaces \
                     one rather than repairing this"
                ),
                "a marker's content is the spelling of an identity and this is not".to_owned(),
                "not the 16 lowercase hexadecimal characters a root's identity is spelled as"
                    .to_owned(),
                "expected 16 hex characters, found 15".to_owned(),
            ],
        );
    }

    // EL-1, EP-13: a mapped root is a folder somebody keeps their own files in,
    // so every refusal registration makes tells the person standing at the
    // device which folder to go and look at, and tells the diagnostic event
    // only which state of coffret's own folder it was.
    #[test]
    fn the_marker_refusals_name_the_root_for_a_person_and_never_for_the_log() {
        const ROOT: &str = "/home/someone/Pictures/Holidays";
        // A folder of the person's own that a case-folding volume does not tell
        // apart from coffret's (spec: EP-14), for the one refusal that names
        // such a folder as well as the root.
        const FOLDED: &str = ".COFFRET";

        // What the entropy source said, for the one refusal that carries such a
        // cause. Its rendering is composed from the constant rather than written
        // out, so a reworded upstream sentence is not a failure of this layer.
        let unavailable = getrandom::Error::UNSUPPORTED;

        let refusals = [
            (
                Error::ManagementAreaNotADirectory {
                    root: PathBuf::from(ROOT),
                },
                "Device::ManagementAreaNotADirectory".to_owned(),
            ),
            (
                Error::ManagementAreaIncomplete {
                    root: PathBuf::from(ROOT),
                },
                "Device::ManagementAreaIncomplete".to_owned(),
            ),
            // The one of them about a folder of the person's rather than
            // coffret's, so it names two things to them — the root and the
            // folder standing in it — and neither reaches the event.
            (
                Error::ManagementAreaFolded {
                    root: PathBuf::from(ROOT),
                    name: FOLDED.to_owned(),
                },
                "Device::ManagementAreaFolded".to_owned(),
            ),
            (
                Error::MarkerNotARegularFile {
                    root: PathBuf::from(ROOT),
                },
                "Device::MarkerNotARegularFile".to_owned(),
            ),
            (
                Error::MarkerMalformed {
                    root: PathBuf::from(ROOT),
                    // Through the reading itself, which is the only thing that
                    // makes one of these: what is wrong with the content is not
                    // a shape this layer gets to state.
                    cause: coffret_usecase::root_marker::parse(b"not an identity")
                        .expect_err("that content names no identity"),
                },
                "Device::MarkerMalformed(defect=not an identity)".to_owned(),
            ),
            (
                Error::RootMarkerNotDrawn {
                    root: PathBuf::from(ROOT),
                    cause: coffret_format::Error::EntropyUnavailable { cause: unavailable },
                },
                format!(
                    "Device::RootMarkerNotDrawn: Format: could not draw random bytes: \
                     {unavailable}"
                ),
            ),
            // The one the marker's *reader* makes rather than its writer, and it
            // owes the same two things: the folder to the person, and the shape
            // of the wrong folder to the event.
            (
                Error::RootRefused(RefusedRoot {
                    prefix: Some(entry_path("albums")),
                    local_root: PathBuf::from(ROOT),
                    reason: RootRefused::MarkerMismatch,
                }),
                "Device::RootRefused: MarkerMismatch".to_owned(),
            ),
        ];

        for (error, rendered) in refusals {
            let said = error.to_string();
            assert!(
                said.contains(ROOT),
                "the person is told which folder it is about: {said}"
            );
            assert_eq!(error.redacted(), rendered);
            assert!(
                !error.redacted().contains(ROOT),
                "and the event carries no part of it: {}",
                error.redacted()
            );
            assert!(
                !error.redacted().contains(FOLDED),
                "nor any name standing in it: {}",
                error.redacted()
            );
        }
    }

    // EL-1, EP-13: the reading's other outcome, where the marker settles nothing
    // because the operating system would not answer. The person is owed the same
    // two things as above and one more — which of coffret's own names inside the
    // folder refused. A reading passes through two of them, and they are
    // different things to go and look at: a sentence that named the marker for a
    // refusal met on the folder holding it would send somebody to a file whose
    // own mode is sound. The event carries the operation and the kind and no
    // part of either path.
    #[test]
    fn a_root_that_could_not_be_asked_about_names_what_refused_and_never_the_folder() {
        const ROOT: &str = "/home/someone/Pictures/Holidays";

        let refused = |operation, path: String| Error::RootUnvouched {
            local_root: PathBuf::from(ROOT),
            cause: LocalIoError::new(
                operation,
                path,
                io::Error::from(io::ErrorKind::PermissionDenied),
            ),
        };

        let area = refused(LocalOperation::Stating, format!("{ROOT}/{MANAGEMENT_AREA}"));
        let said = area.to_string();
        assert!(
            said.contains(ROOT),
            "the person is told which folder it is about: {said}",
        );
        assert!(
            said.contains(&format!("{MANAGEMENT_AREA} in it could not be stated")),
            "the folder holding the marker is what refused, and it is what is named: {said}",
        );
        assert!(
            !said.contains(&format!("{MANAGEMENT_AREA}/{MARKER_FILE}")),
            "nothing was asked of the marker itself here: {said}",
        );

        let marker = refused(
            LocalOperation::Reading,
            format!("{ROOT}/{MANAGEMENT_AREA}/{MARKER_FILE}"),
        );
        assert!(
            marker.to_string().contains(&format!(
                "{MANAGEMENT_AREA}/{MARKER_FILE} in it could not be read"
            )),
            "and the marker where that is what refused: {marker}",
        );

        assert_eq!(
            marker.redacted(),
            "Device::RootUnvouched: Local::Io(operation=read, kind=PermissionDenied)",
        );
        assert!(
            !marker.redacted().contains(ROOT),
            "the event carries no part of it: {}",
            marker.redacted(),
        );
    }
}
