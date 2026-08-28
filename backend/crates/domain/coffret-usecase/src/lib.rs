//! What the use-case layer asks of the world outside it.
//!
//! Coffret keeps a Library on Storage it does not trust, and the format layer
//! that turns user data into Storage Objects knows nothing about where those
//! objects go. This crate is the seam between them, and it names two ports.
//!
//! [`ObjectStore`] is the one every Storage provider is reached through, and
//! the vocabulary around it — [`ObjectRef`], [`CommitSlot`], [`ObjectInfo`],
//! [`Error`] — is the storage vocabulary the rest of the backend reasons in.
//!
//! [`Index`] is the device-local catalog of one Library: which Container holds
//! the Entry at each Entry Path, which Containers are current, and — kept
//! strictly apart, in [`device_state`] — what only this device knows about its
//! own folders and spools. It is a cache and never the source of truth, so it
//! is defined in terms of what a restore and a replay do to it
//! ([`SnapshotContent`], [`JournalRecord`]) rather than in terms of a store,
//! and it fails in a vocabulary of its own, [`IndexError`].
//!
//! The storage port is deliberately narrow, because Storage is only ever handed
//! ciphertext and only ever asked to keep it, hand it back, enumerate it, and
//! remove it. Two things it does fix, because the Library's correctness rests on
//! them rather than on any one provider:
//!
//! - **Conditional create.** [`ObjectStore::reserve_create`] and
//!   [`ObjectStore::put_if_absent`] are how a commit is won or lost: of several
//!   writers spending one slot exactly one succeeds, and the rest get
//!   [`Error::AlreadyExists`] — a state, not a transport hiccup, and
//!   [`ObjectStore::object_at`] is how a loser reaches what took the slot.
//! - **Two kinds of removal.** [`ObjectStore::trash`] is recoverable, which is
//!   what removing a Container means; [`ObjectStore::purge`] is irreversible and
//!   read-back verified, which is what Master Key rotation needs of old-epoch
//!   control objects.
//!
//! The crate performs no I/O: it names the operations, the values they exchange,
//! and the failures they may report. Talking to Google Drive or to S3 is a
//! gateway's business, and each gateway translates its provider's errors into
//! [`Error`] so that callers never read a provider message to decide what
//! happened — including whether another attempt could succeed, which
//! [`Error::is_retryable`] answers from the type alone. [`RetryPolicy`] is what
//! acts on that answer, and it is here rather than in a gateway because when to
//! stop trying is one decision for the whole backend rather than one per
//! provider.
//!
//! [`ControlHead`] sits just above that port: it derives from a control head the
//! slot its successor commits into and the slot its checkpoint goes in.
//!
//! [`commit`] is the one place the two ports and the format layer meet: it takes
//! a batch whose Containers are already on Storage and carries it through the
//! commit protocol until the Library's current state is what the batch says.
//!
//! [`sync`] is what produces such a batch from a folder on this device: it
//! scans the mapped folders against the Index, encodes what changed, uploads
//! it, and hands the result to [`commit`].
//!
//! [`freeze`] produces one too, and it is the shape a Library is meant to be
//! kept in: a sync leaves one Storage Object per file, which puts a folder of
//! ten thousand images past what a provider or a Library-wide rebuild wants to
//! walk, and a freeze packs the eligible files into Packs of consecutive Entries
//! instead (spec: PK-1, PK-7). It is the one flow whose Containers are larger
//! than memory, which is why it writes them through
//! [`ContainerWriter`](coffret_format::ContainerWriter) rather than
//! [`encode`](coffret_format::encode).
//!
//! [`fetch`] is the same journey read backwards, and the other half of the round
//! trip: it catches the Index up, opens the committed Keyring, pulls the
//! Containers this device's mapped folders are missing, verifies them, and writes
//! the files into place. Together they are what "this folder is in the
//! Library" means from either end — a device uploads an Entry or fetches it, and
//! EP-10 names those as the two ways one is materialized at all.
//!
//! Those three are the parts of the crate that touch the local filesystem, which
//! is not a port for the reason a device's own disk is not Storage — nothing
//! there is behind the trust boundary the ports exist to cross. They are also
//! the crate's only modules that perform a sequence rather than naming a
//! contract, and they are why the crate depends on `coffret-format` at all.
//!
//! Behind the `conformance` feature, the `conformance`, `index_conformance`,
//! `commit_conformance`, `sync_conformance`, `freeze_conformance`, and
//! `fetch_conformance` modules are those contracts as suites of tests every
//! adapter runs, so a second adapter cannot quietly redefine what a port — or
//! what a commit, a sync, a freeze, or a fetch over both of them — means.
//! `InMemoryStore` and `InMemoryIndex` are what to drive them — and the crate's
//! own cases — against without a provider, a container, or a file. This crate
//! runs all six suites against those two. None of the eight is linked here,
//! because they are not in the documentation this crate builds without that
//! feature.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod byte_stream;
pub use byte_stream::ByteStream;

pub mod commit;

mod commit_slot;
pub use commit_slot::CommitSlot;

// The commit flow's own contract, behind the same feature as the two ports'.
#[cfg(feature = "conformance")]
pub mod commit_conformance;

mod committed_batch;
pub use committed_batch::CommittedBatch;

#[cfg(feature = "conformance")]
pub mod conformance;

// What every suite over a flow that writes user data ends at: reading the
// Library back out of Storage the way a device with no Index would. Test support
// rather than product code, behind the same feature as the suites that share it.
#[cfg(feature = "conformance")]
mod conformance_library;

mod control_head;
pub use control_head::ControlHead;

pub mod device_state;

mod error;
pub use error::{Error, Result};

pub mod fetch;

// The fetch's own contract, behind the same feature as the other four.
#[cfg(feature = "conformance")]
pub mod fetch_conformance;

pub mod freeze;

// The freeze's own contract, behind the same feature as the other five.
#[cfg(feature = "conformance")]
pub mod freeze_conformance;

mod index;
pub use index::Index;

// The `Index` contract as a suite, behind the same feature as the storage
// port's, and for the same reason: only a test target needs it.
#[cfg(feature = "conformance")]
pub mod index_conformance;

mod index_error;
pub use index_error::{IndexError, IndexResult};

// What the flows that touch this device's disk need and none of them owns: the
// keys one Master Key epoch's Containers are sealed and opened with, the word
// for what a local file or folder was being asked for when the operating system
// refused, the reading of a local file's modification time, the form an Entry
// Path is spelled in once text from outside the Library has become one
// (spec: EP-1), the walk of the mapped folders itself, and the finding that a
// mapped root says nothing about the Library at all (spec: EP-12) — which is one
// finding whichever flow made it, because both of them walk the same roots. The
// three that are public are re-exported from each flow, where their callers
// already reach for the rest of the vocabulary.
mod library_keys;
pub use library_keys::LibraryKeys;

mod local_error;

mod local_mtime;

mod local_operation;
pub use local_operation::LocalOperation;

mod local_scan;

mod nfc;

mod unavailable_root;
pub use unavailable_root::{RootUnavailable, UnavailableRoot};

// Test support rather than product code: the crate's own tests need a store and
// a catalog to drive, and a gateway building either conformance suite may want
// one to compare against.
#[cfg(any(test, feature = "conformance"))]
mod in_memory_index;
#[cfg(any(test, feature = "conformance"))]
pub use in_memory_index::InMemoryIndex;

#[cfg(any(test, feature = "conformance"))]
mod in_memory_store;
#[cfg(any(test, feature = "conformance"))]
pub use in_memory_store::InMemoryStore;

mod object_info;
pub use object_info::ObjectInfo;

mod object_page;
pub use object_page::ObjectPage;

// The handle a store names an object with is domain vocabulary rather than
// storage-port vocabulary — the Index caches one per current Container — so it
// lives in `coffret-model` and is re-exported here, where the callers of the
// port already reach for it. What a control object carries is domain
// vocabulary for the same reason: `coffret-format` encodes those three values
// and this port speaks them, so neither layer owns them.
pub use coffret_model::{ContainerAddition, JournalRecord, ObjectRef, SnapshotContent};

mod object_store;
pub use object_store::ObjectStore;

mod page_token;
pub use page_token::PageToken;

mod provider_hash;
pub use provider_hash::ProviderHash;

mod retry;
pub use retry::RetryPolicy;

mod scratch;

// What a sync and a freeze both do once their Container exists: write it to the
// spool with its digests folded in, hand it to the upload, and put it in the
// batch a commit takes.
mod spool_file;

mod spooled_container;

pub mod sync;

// The folder sync's own contract, behind the same feature as the two ports' and
// the commit flow's.
#[cfg(feature = "conformance")]
pub mod sync_conformance;

mod upload;
