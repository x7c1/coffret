//! What can go wrong keeping a Library on this device.
//!
//! The type, its variants and the constructors this crate raises it through
//! are here; what each variant says is in [`display`], what it says underneath
//! in [`source`], what a diagnostic event may be told in [`redacted`], and
//! what a lower layer's failure becomes in [`from`]. The values a variant
//! carries to say which name was refused, which step failed, which two OAuth
//! clients differ and why a promotion was refused are [`NameDefect`],
//! [`CreationStep`], [`ClientMismatch`] and [`PromotionObstacle`], each in a
//! module of its own.
//!
//! Whether one is a code exchange refused to a client that sent no secret is
//! asked in [`exchange_without_client_secret`].

use std::error;
use std::io;
use std::path::PathBuf;

use coffret_model::EntryPath;
use coffret_usecase::commit::CommitError;
use coffret_usecase::fetch::{BelowRootError, DescentError, FetchError};
use coffret_usecase::freeze::FreezeError;
use coffret_usecase::root_marker::MalformedMarker;
use coffret_usecase::sync::SyncError;
use coffret_usecase::{LocalIoError, LocalOperation, RefusedRoot};

mod client_mismatch;
pub use client_mismatch::ClientMismatch;

mod creation_step;
pub use creation_step::CreationStep;

mod display;

mod exchange_without_client_secret;

mod from;

mod name_defect;
pub use name_defect::NameDefect;

mod promotion_obstacle;
pub use promotion_obstacle::PromotionObstacle;

mod redacted;

mod source;

#[cfg(test)]
mod tests;

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
///
/// A catalog that could not be used is [`Index`](Self::Index), whichever entry
/// point met it: recording a mapping, a fetch, and asking where a file belongs
/// all report it under that one name, so a caller saying "the catalog could not
/// be used" matches one shape. Creating or joining a Library reports it as the
/// cause of [`LibraryNotCreated`](Self::LibraryNotCreated) or
/// [`LibraryNotJoined`](Self::LibraryNotJoined) at [`CreationStep::Index`],
/// because what a person is owed there first is which step undid the attempt;
/// the cause underneath is the same [`Index`](Self::Index). The fetch's
/// vocabulary carries the same failure as a variant of its own, because its
/// flows reach the catalog with `?` at every step, and the choice here is to
/// take it out of that vocabulary at this crate's door rather than to move it
/// out of `coffret-usecase`. The move would be the more honest shape — a
/// catalog that would not open is nothing the fetch decided — but it would
/// change the return type of every fetch flow and of every caller of them for a
/// failure none of them decides anything about, and it would still leave the
/// commit's own catalog failure inside a fetch that met one while catching up.
/// Lifting it at the door costs one function, used by every conversion out of
/// that vocabulary, so the variants below that carry a [`FetchError`] carry
/// only the fetch's own verdicts.
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
    /// The name given for an account is not one an account can have
    /// (spec: SA-8).
    ///
    /// It becomes a directory name and is bound into every envelope of the
    /// account, so it is held to one rule, and the refusal says what that rule
    /// is rather than which part of it was missed.
    InvalidAccountName {
        /// The name that was asked for.
        name: String,
    },
    /// The device holds more than one account and no name said which one a
    /// Library is to reference (spec: SA-8).
    AccountNameRequired {
        /// How many accounts the device holds.
        held: usize,
    },
    /// None of the accounts this device holds reaches the app folder a join
    /// named, and the new account the join would consent as needs a name
    /// because the device already holds one (spec: SA-8).
    NoAccountReachesFolder,
    /// A Library names an OAuth client other than the one the account it
    /// references was consented to (spec: SA-8).
    ///
    /// Boxed for its width; see [`ClientMismatch`].
    ClientMismatch(Box<ClientMismatch>),
    /// The account-cache key envelope of a Library that references an account
    /// is missing, malformed, or fails to authenticate (spec: SA-9, KD-12).
    ///
    /// Never reported as a Library that references no account: a damaged
    /// envelope is not quietly answered with a second consent.
    UnreadableAccountEnvelope {
        /// The Library whose envelope it is.
        library: String,
        /// The account the Library references.
        account: String,
        /// What the format layer made of the file, where there was one.
        ///
        /// Boxed so that the format crate's enum does not set this one's width.
        cause: Option<Box<coffret_format::Error>>,
    },
    /// No Library that references an account opens with the Passphrase given,
    /// so the account's grant cannot be reached (spec: SA-9).
    ///
    /// The account-cache key is kept only in the envelopes of the Libraries
    /// that reference the account, each under its own Master Key, so reaching
    /// it takes unlocking one of them.
    AccountNotOpened {
        /// The account that was asked for.
        account: String,
        /// The Library whose Passphrase would open it: one that references it.
        library: String,
    },
    /// A Library's previous per-Library grant would go into an account the
    /// device already holds, and that account cannot take it in, for the
    /// reason [`PromotionObstacle`] says (spec: SA-8).
    PromotionNeedsName {
        /// The Library whose grant it is.
        library: String,
        /// The account the promotion would have referenced.
        account: String,
        /// Why the held account cannot take it in.
        obstacle: PromotionObstacle,
    },
    /// No account of this name is on this device.
    NoSuchAccount {
        /// The account that was asked for.
        account: String,
    },
    /// A Library already references an account, and another was named for it.
    ///
    /// The name is bound into every envelope of the account (spec: SA-9), so a
    /// Library moving to another name is a re-seal no command performs yet.
    AccountFixed {
        /// The Library that was named.
        library: String,
        /// The account it references.
        account: String,
        /// The account that was asked for instead.
        requested: String,
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
        ///
        /// Boxed, as every flow verdict this enum carries is — the four flows',
        /// and the fetch's vocabulary wherever a gesture that fetched nothing
        /// reports it. Each of them is wider than anything else here, and a
        /// verdict carried inline sets the width of every `Result` this crate
        /// returns, a name that could not be a directory included. The pointer
        /// keeps that cost on the refusals that have a verdict to carry, and
        /// the size test in this module is what notices the next one that
        /// does not.
        cause: Box<SyncError>,
    },
    /// A freeze did not finish.
    Freeze {
        /// What the flow reported.
        ///
        /// Boxed for the reason [`Sync`](Self::Sync)'s is.
        cause: Box<FreezeError>,
    },
    /// A fetch did not finish.
    ///
    /// Never a catalog that could not be used: the fetch's vocabulary names
    /// that as one of its own variants, and it is taken out of the value at
    /// this crate's door and reported as [`Index`](Self::Index), which is where
    /// every other entry point reports it.
    Fetch {
        /// What the flow reported.
        ///
        /// Boxed for the reason [`Sync`](Self::Sync)'s is.
        cause: Box<FetchError>,
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
    /// door onto it. A catalog that could not be read is not among them — it
    /// settled nothing about the path either way, and it is
    /// [`Index`](Self::Index) instead.
    LocalPathNotSettled {
        /// What the translation reported.
        ///
        /// Boxed for the reason [`Sync`](Self::Sync)'s is.
        cause: Box<FetchError>,
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
    /// keeps it true of every refusal that reaches here. A catalog that could
    /// not be read decided nothing about the file and is
    /// [`Index`](Self::Index) instead. Which refusal it was is the `cause`'s to
    /// say, in the fetch's vocabulary because where a file may stand on this
    /// device is written once and the flow that places files is where
    /// (spec: EP-4, EP-9).
    FileNotTakenIn {
        /// What the write into the mapped folder reported.
        ///
        /// Boxed for the reason [`Sync`](Self::Sync)'s is.
        cause: Box<FetchError>,
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
    /// what the reader was owed and is true of every refusal carried here. A
    /// catalog that could not be read decided nothing about the folder and is
    /// [`Index`](Self::Index) instead. Which refusal it was is the `cause`'s to
    /// say, in the fetch's vocabulary because which folder of this device
    /// stands for a part of the Library is written once (spec: EP-9).
    LocalFilesNotRead {
        /// What the read of the mapped folder reported.
        ///
        /// Boxed for the reason [`Sync`](Self::Sync)'s is.
        cause: Box<FetchError>,
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
    /// stand for, and a row that outlived the Entry it was written for, which is
    /// a state to go on from rather than one to fail at (spec: EP-10). A catalog
    /// that could not be read decided nothing about the file and is
    /// [`Index`](Self::Index) instead. Which of them it was is the `cause`'s to
    /// say, in the fetch's vocabulary because the translation it went through is
    /// the fetch's own (spec: EP-9).
    LocalFileNotOpened {
        /// What the translation reported.
        ///
        /// Boxed for the reason [`Sync`](Self::Sync)'s is.
        cause: Box<FetchError>,
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
        ///
        /// Boxed for the reason [`Sync`](Self::Sync)'s is.
        cause: Box<CommitError>,
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

impl Error {
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
    /// and not on the same verdict: the marker stands in the root every one of
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

    /// The fetch's vocabulary reported under `verdict`, unless what it carries
    /// is the catalog failing.
    ///
    /// That one is [`Index`](Self::Index) whichever door it came through: a
    /// catalog that could not be read decided nothing about the path, the file,
    /// or the transfer the other variants are about, and a caller asking "could
    /// the catalog be used" is owed one shape to match rather than one per
    /// gesture. Every conversion out of the fetch's vocabulary in this crate goes
    /// through here — `?` included — which is what keeps a
    /// [`FetchError::Index`] out of every variant that carries a
    /// [`FetchError`].
    pub(crate) fn index_or(cause: FetchError, verdict: impl FnOnce(FetchError) -> Self) -> Self {
        match cause {
            FetchError::Index(cause) => Self::Index { cause },
            cause => verdict(cause),
        }
    }

    /// The EP-9 translation's verdict, asked on its own (see
    /// [`LocalPathNotSettled`](Self::LocalPathNotSettled)).
    pub(crate) fn local_path_not_settled(cause: FetchError) -> Self {
        Self::index_or(cause, |cause| Self::LocalPathNotSettled {
            cause: Box::new(cause),
        })
    }

    /// A refusal met with somebody's file in hand (see
    /// [`FileNotTakenIn`](Self::FileNotTakenIn)).
    pub(crate) fn file_not_taken_in(cause: FetchError) -> Self {
        Self::index_or(cause, |cause| Self::FileNotTakenIn {
            cause: Box::new(cause),
        })
    }

    /// A refusal met reading what somebody has put in a mapped folder (see
    /// [`LocalFilesNotRead`](Self::LocalFilesNotRead)).
    pub(crate) fn local_files_not_read(cause: FetchError) -> Self {
        Self::index_or(cause, |cause| Self::LocalFilesNotRead {
            cause: Box::new(cause),
        })
    }

    /// A refusal met opening the file this device placed for an Entry (see
    /// [`LocalFileNotOpened`](Self::LocalFileNotOpened)).
    pub(crate) fn local_file_not_opened(cause: FetchError) -> Self {
        Self::index_or(cause, |cause| Self::LocalFileNotOpened {
            cause: Box::new(cause),
        })
    }

    /// What [`descent`](Self::descent) says, for a step taken below a root a
    /// descent has already vouched for.
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
                cause: Box::new(FetchError::UnmaterializablePath {
                    path: path.clone(),
                    stopped_at: Some(stopped_at),
                }),
            },
            BelowRootError::Io(refused) => Self::Local(refused),
        }
    }
}
