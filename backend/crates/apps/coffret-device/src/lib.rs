//! What one device keeps for a Library, and how it opens one.
//!
//! Every flow coffret performs already exists as a library call — the sync, the
//! freeze, the fetch, the catalog, the two gateways, the byte forms and the
//! keys — and until this crate nothing composed them. That is what this is: the
//! composition root, the place where "a Library on this device" stops being an
//! argument list and becomes a directory with a Master Key, a catalog, a spool,
//! and a note of where its Storage is.
//!
//! It is a library and not a command for one reason. The browser-based explorer
//! will do the same things — open a Library, record a mapping, eventually
//! create one — and two implementations of "where a device keeps a Library"
//! would disagree the first time either changed. The command line is a shell
//! over this; so is the explorer.
//!
//! # One directory per Library
//!
//! Under the state directory the platform names — `$XDG_STATE_HOME`, or
//! `$HOME/.local/state` where that is unset:
//!
//! ```text
//! coffret/libraries/<name>/
//!   settings.json      where the Library lives (this crate's contract)
//!   master-key.cfmk    the Master Key under the Passphrase (spec: KD-9)
//!   token-cache.cftc   the sealed OAuth grant (spec: KD-10), Drive only
//!   index.sqlite       the catalog
//!   spool/             encrypted Containers waiting to be uploaded
//! ```
//!
//! `<name>` is what this device calls the Library, not what the Library calls
//! itself: another device holding the same Library may call it something else,
//! the way it may map its folders differently (spec: CK-7). The four files and
//! the directory are created owner-only, and none of them is named in
//! `settings.json` — the layout is the single answer to where each piece is.
//!
//! [`STATE_DIRECTORY`] stands in for the `coffret` directory itself rather than
//! for what is above it, so a Library of a run under it is at
//! `$COFFRET_STATE_DIR/libraries/<name>/` and not under a second `coffret`.
//!
//! # What the settings file is for
//!
//! [`DeviceSettings`] is a contract rather than a convenience: the explorer will
//! read the same file, so it is versioned, and a version this build does not
//! know is refused rather than repaired. It holds two things — which Library
//! this is, and where on Storage it is — and no credential a device could not
//! obtain again for itself.
//!
//! # Two ways a Library appears here, and five things a device does with one
//!
//! [`create_library`] draws a Master Key, stores it under the Passphrase, and
//! hands back the [`RecoveryCode`] — the one copy of that key which exists off
//! the device. [`join_library`] is the same directory built from such a code:
//! the Master Key is entered rather than drawn, and nothing is written to
//! Storage, because the Library is already there.
//!
//! [`run_sync`], [`run_freeze`], [`run_fetch`] and [`run_fetch_entry`] are the
//! flows over a Library that exists. Each one opens the Library, supplies the
//! two values a device provides rather than derives — what it calls this batch
//! and what its clock says — and runs the use case. Which folders any of them
//! touches is never an argument: that is the device's mappings, which the
//! catalog holds (spec: EP-9).
//!
//! [`run_catch_up`] is the fifth and the smallest: it brings the catalog to the
//! Library's head and stops there (spec: CK-9), which is the first step of the
//! other four on its own. Nothing is scanned, uploaded, fetched or placed, so a
//! device that has just joined learns what the Library holds without a single
//! Container being read — and every row it learns of is `remote` until somebody
//! asks for one (spec: EP-10).
//!
//! # One unlock, or one process
//!
//! Each of those five is one unlock and one run, which is what a command does.
//! A process that stays up — the explorer's server — opens the Library once and
//! runs many things over it, so every flow also exists as a method on
//! [`OpenLibrary`]: [`sync`](OpenLibrary::sync), [`freeze`](OpenLibrary::freeze),
//! [`fetch`](OpenLibrary::fetch), [`fetch_entry`](OpenLibrary::fetch_entry) and
//! [`catch_up`](OpenLibrary::catch_up) are the bodies, and the five `run_` calls
//! are `open_library` followed by one of them. There is one body per flow, so
//! neither shell can drift from the other.
//!
//! An open Library also answers what a person browsing one asks, out of the
//! catalog and without touching Storage: [`folders`](OpenLibrary::folders) and
//! [`list`](OpenLibrary::list) read the Library as folders (spec: EP-2),
//! [`state_of`](OpenLibrary::state_of) says whether this device has one Entry's
//! file (spec: EP-10), and [`local_path_of`](OpenLibrary::local_path_of) says
//! where that file belongs (spec: EP-9). [`EntryFetches`] is what a process
//! serving more than one reader wraps [`fetch_entry`](OpenLibrary::fetch_entry)
//! in, so two readers asking for one Entry at once fetch it once.
//!
//! And it takes a file the other way. [`receive_file`](OpenLibrary::receive_file)
//! writes a file somebody handed this device into the folder the mappings put it
//! in (spec: EP-9), whole or not at all (spec: EP-11) — which is the same gesture
//! as copying it in by hand, and is carried into the Library by the same
//! [`sync`](OpenLibrary::sync). Until one has run, such a file is on this device
//! and not in the Library, which is what
//! [`added_locally`](OpenLibrary::added_locally) reads a mapped folder for and
//! [`added_at`](OpenLibrary::added_at) answers about one path.
//!
//! # Reading what a run answered
//!
//! A run that returns `Ok` has not necessarily backed up or placed everything,
//! and every outcome says so in its own words. [`Findings`] is the one view over
//! all of them — the files a run left alone, the mapped roots it could not
//! vouch for, the Containers it has no key for, the batches it settled — so that
//! the command line and the explorer read the same answer rather than each
//! choosing which half to show (spec: PK-14, EP-11, EP-12).
//!
//! # The Passphrase, and what it does not reach
//!
//! [`open_library`] spends the Passphrase once, derives what the flows need, and
//! drops the key when the process ends: one process is one unlock (spec: DK-9).
//!
//! Every call that needs a Passphrase takes it as a callback rather than as a
//! value, and calls it only once every refusal that needs no key has passed. A
//! name that is not one path component, a Library that is not on this device, a
//! bucket that does not answer: none of those is worth a person typing a
//! Passphrase twice to be told about.
//!
//! Recording a mapping needs no Passphrase at all, because a mapping is device
//! state in a plaintext catalog and says nothing the Library keeps secret
//! (spec: CK-7).

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod add;
pub use add::{AddedFile, IncomingFile};

mod authorize;
pub use authorize::authorize;

// The moment a run stands at and the name it gives its batch: two values every
// flow supplies rather than derives, and neither of them anything a caller
// should have to invent (spec: OC-2, CP-7).
mod batch_id;

mod browse;
pub use browse::{ChildFolder, EntryState, FileRow, FolderListing};

mod create_library;
pub use create_library::{create_library, CreateLibraryRequest, CreatedLibrary, NewProvider};

mod device_settings;
pub use device_settings::{DeviceSettings, ProviderSettings};

// The three things every Drive flow here is built from, kept in one place so
// that the cache one command writes is the cache the next one reads.
mod drive;

mod entry_fetches;
pub use entry_fetches::EntryFetches;

mod error;
pub use error::{CreationStep, Error, NameDefect, Result};

mod finding;
pub use finding::Finding;

mod finding_reason;
pub use finding_reason::FindingReason;

mod findings;
pub use findings::Findings;

// Reading one folder of the Library out of Entry Paths: what is inside it, and
// what a child of it is called. Two rules of EP-2's separator, in one place,
// because the listing and the files added beside it have to draw the same
// boundary or a row falls between them.
mod folder_paths;

mod join_library;
pub use join_library::{join_library, JoinLibraryRequest, JoinedLibrary, JoinedProvider};

mod library_dir;
pub use library_dir::{LibraryDir, STATE_DIRECTORY};

// The three things both ways of putting a Library on this device write once its
// Master Key and its place on Storage are settled.
mod library_files;

// Where one Entry's file belongs on this device, which is EP-9 asked of the use
// case rather than answered again here.
mod local_path;

mod mapping;
pub use mapping::{mappings, set_mapping};

// Whether a piece of text is one path component. Both the name a Library has on
// this device and a mapping's prefix are held to that shape, so the check
// belongs to neither of them.
mod name_defect;

mod open_library;
pub use open_library::{open_library, OpenLibrary};

// How a device's own files are created: owner-only, and through a rename, so an
// interrupted write leaves what was there rather than half of what replaces it.
mod owner_only;

mod recovery_code;
pub use recovery_code::recovery_code;

mod run_catch_up;
pub use run_catch_up::run_catch_up;

mod run_fetch;
pub use run_fetch::run_fetch;

mod run_fetch_entry;
pub use run_fetch_entry::run_fetch_entry;

mod run_freeze;
pub use run_freeze::run_freeze;

mod run_sync;
pub use run_sync::run_sync;

// Reaching an S3 bucket from what a device recorded about it, which both opening
// a Library and asking whether its bucket is there are built from.
mod s3;

// Where a Library directory is built before it takes the name it is known by,
// shared by the two flows that build one.
mod staging;

mod stored_master_key_file;
pub use stored_master_key_file::StoredMasterKeyFile;

// What this crate's own tests build a Library from, in one place so that the
// state directory the environment names is set once for the whole binary.
#[cfg(test)]
mod testing;

// What a shell over this crate needs to name and would otherwise have to reach
// past it for: the values these calls take and hand back. The Recovery Code is
// what `create_library` produces, a mapping is what `mappings` returns, an Entry
// Path is what narrows a freeze or a fetch, the five outcomes are what the
// flows answer with, and a commit outcome is what two of them carry to say the
// Library changed. The modification time and the Container kind are what a
// listing's rows carry, and the fetch's, the sync's and the catch-up's own
// refusals and the fetch's finding are what [`Error::Fetch`], [`Error::Sync`],
// [`Error::CatchUp`] and [`EntryFetch::Surfaced`] carry — a shell branching on
// any of them has to be able to name it. None of them belongs to this crate, and
// a shell printing one should not have to take a dependency on the layer that
// owns it — neither the command line nor the explorer's server does.
pub use coffret_format::RecoveryCode;
pub use coffret_model::{ContainerKind, EntryPath, Mtime};
pub use coffret_usecase::catch_up::CatchUpOutcome;
pub use coffret_usecase::commit::{CommitError, CommitOutcome};
pub use coffret_usecase::device_state::Mapping;
pub use coffret_usecase::fetch::{EntryFetch, FetchError, FetchOutcome, Surfaced};
pub use coffret_usecase::freeze::FreezeOutcome;
pub use coffret_usecase::sync::{Reconciled, SyncError, SyncOutcome};
pub use coffret_usecase::RootUnavailable;
