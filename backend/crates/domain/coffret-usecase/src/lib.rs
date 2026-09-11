//! What the use-case layer asks of the world outside it.
//!
//! Coffret keeps a Library on Storage it does not trust, and the format layer
//! that turns user data into Storage Objects knows nothing about where those
//! objects go. This crate is the seam between them, and it names two ports and
//! the capabilities over this device's own disk.
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
//! Those three are the parts of the crate that reach this device's own disk,
//! and each of them reaches it through a named capability. They are also the
//! crate's only modules that perform a sequence rather than naming a contract,
//! and they are why the crate depends on `coffret-format` at all.
//!
//! Storage is behind a port because of the trust boundary: a Library lives on
//! something coffret is only ever willing to hand ciphertext to. The device's
//! own disk is behind nothing of the kind — it is this device's, and the flows
//! read and write plaintext on it — and it is behind capabilities all the same,
//! for a different reason. What the three flows promise about local files are
//! promises about *failure*, *absence*, and *interruption*: a spool is announced
//! before it can exist and disposed of however far its writing got (spec: OC-2,
//! OC-6), a mapped root that is not there says nothing about the Library rather
//! than saying every Entry under it is gone (spec: EP-12), and a placement
//! becomes visible only once its bytes are on the device and its content has
//! been held against the catalog (spec: EP-11). Every one of those rules is
//! about what a run leaves behind when a chosen step refuses, and a real
//! filesystem refuses nothing on request — so the recovery rules can only be
//! tested against a disk that fails where a case says.
//!
//! [`Spool`] is the writing half of that disk, [`MappedRoots`] the reading half,
//! and [`Destinations`] where a fetched Entry is placed. Asking the operating
//! system is the local filesystem gateway's business, as talking to a provider
//! is a Storage gateway's, and it is the one place the flows here ask either.
//! The first two fail in [`LocalIoError`] and the third in [`DescentError`],
//! which carries one; [`SpoolWriter::finish`] and [`ScratchFile::flush`] are
//! what make "written" and "on the device" two different things;
//! [`SourceReader`] is what keeps a Pack's members from having to fit in
//! memory; and [`Destination`] is a folder held open rather than a path,
//! because a path handed back to the operating system is a question asked
//! twice (spec: EP-4).
//!
//! [`catch_up`] is the one that touches neither the filesystem nor Storage's
//! write side. It is the first step of each of the three on its own — replay
//! what the Journal holds and stop there — for the caller that wants to know
//! what the Library has become without bringing any of it over.
//!
//! Behind the `conformance` feature, the `conformance`, `index_conformance`,
//! `spool_conformance`, `mapped_roots_conformance`, `destinations_conformance`,
//! `commit_conformance`, `sync_conformance`, `freeze_conformance`, and
//! `fetch_conformance` modules are those contracts as suites of tests every
//! adapter runs, so a second adapter cannot quietly redefine what a port or a
//! capability — or what a commit, a sync, a freeze, or a fetch over them —
//! means. `InMemoryStore`, `InMemoryIndex`, and `InMemoryFs` are what to drive
//! them — and the crate's own cases — against without a provider, a container,
//! or a file. This crate runs all nine suites against those three. None of the
//! twelve is linked here, because they are not in the documentation this crate
//! builds without that feature.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod byte_stream;
pub use byte_stream::ByteStream;

pub mod catch_up;

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

// Where this crate's tests and conformance suites turn a literal pair of
// numbers into an Entry's extent, beside the module that does the same for an
// Entry Path and for the same reason.
#[cfg(any(test, feature = "conformance"))]
mod entry_extents;

// Where this crate's tests and conformance suites turn a literal into an
// Entry Path, in one place so that a mistyped fixture is reported as the
// fixture mistake it is.
#[cfg(any(test, feature = "conformance"))]
mod entry_paths;

// Where this crate's tests and conformance suites turn a literal number into a
// generation, beside the two modules above and for the same reason.
#[cfg(any(test, feature = "conformance"))]
mod generations;

// And where they turn one into a ciphertext length claim, in a module of its
// own for the same reason the three above are three.
#[cfg(any(test, feature = "conformance"))]
mod ciphertext_len_claims;

mod error;
pub use error::{Error, Result};

mod missing;
pub use missing::Missing;

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
// refused, the walk of the mapped folders itself, and the finding that a mapped
// root says nothing about the Library at all (spec: EP-12) — which is one
// finding whichever flow made it, because both of them walk the same roots. The
// public ones are re-exported from each flow, where their callers already reach
// for the rest of the vocabulary.
mod library_keys;
pub use library_keys::LibraryKeys;

mod local_error;

// What one operation on this device's own disk failed with, as a value any
// caller outside this crate can build — a gateway behind a capability,
// or a composition root keeping files of its own on the same disk: the
// vocabulary every capability over the local filesystem answers in, and what
// `LocalError::Io` carries.
mod local_io_error;
pub use local_io_error::LocalIoError;

mod local_operation;
pub use local_operation::LocalOperation;

mod mapped_relative_location;
pub use mapped_relative_location::MappedRelativeLocation;

mod local_scan;

// The reading half of what this device's own disk is asked for, beside the
// writing half `Spool` names: what a mapped root is, what one folder holds, and
// one file's plaintext a buffer at a time. Every decision about what any of it
// *means* stays in `local_scan`.
mod mapped_roots;
pub use mapped_roots::MappedRoots;

mod folder_entry;
pub use folder_entry::FolderEntry;

mod folder_entry_kind;
pub use folder_entry_kind::FolderEntryKind;

mod root_probe;
pub use root_probe::RootProbe;

mod source_reader;
pub use source_reader::SourceReader;

// And the third of the three: where a fetched Entry is written, as a folder
// reached without following a link rather than as a path. The folder held open,
// the file written inside it and the flushed file published from that are what
// it hands out, and what a refusal along the way means and what was found
// standing at a name are its own vocabulary.
mod destinations;
pub use destinations::Destinations;

mod destination;
pub use destination::Destination;

mod scratch_file;
pub use scratch_file::ScratchFile;

mod flushed_file;
pub use flushed_file::FlushedFile;

mod descent_error;
pub use descent_error::DescentError;

mod standing;
pub use standing::Standing;

// The mapped-roots capability's own contract, behind the same feature as the
// spool capability's.
#[cfg(feature = "conformance")]
pub mod mapped_roots_conformance;

// And the destinations capability's, behind the same feature as the other two.
#[cfg(feature = "conformance")]
pub mod destinations_conformance;

mod unavailable_root;
pub use unavailable_root::{RootUnavailable, UnavailableRoot};

// The other question a mapped root is asked, and not the one above: EP-12 asks
// whether the root is there to be read from, EP-13 whether the folder standing
// at it is the one that was registered.
mod refused_root;
pub use refused_root::{RefusedRoot, RootRefused};

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

// And a disk to drive them against — the spool, the mapped folders, and the
// places a fetch writes into alike, because one device has one of them. It is
// the one of the three that can be told to fail at a chosen step: what the flows
// promise around the local disk are promises about interruption and about
// absence, and a real filesystem refuses nothing on request (spec: OC-2, OC-6,
// EP-11, EP-12).
#[cfg(any(test, feature = "conformance"))]
mod in_memory_fs;
#[cfg(any(test, feature = "conformance"))]
pub use in_memory_fs::InMemoryFs;

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

// The second name coffret reserves inside a folder a scan walks, beside the
// first (spec: EP-13, EP-14).
pub mod root_marker;

pub mod scratch;

// Where a Container waits between being encoded and being committed, as a
// capability rather than as calls on a filesystem: what the flows promise about
// an interrupted spool can only be held to what the thing underneath them
// actually does when it fails (spec: OC-2, OC-6).
mod spool;
pub use spool::Spool;

// What a sync and a freeze both do once their Container exists: write it to the
// spool with its digests folded in, hand it to the upload, and put it in the
// batch a commit takes.
mod spool_file;

mod spool_writer;
pub use spool_writer::SpoolWriter;

// The spool capability's own contract, behind the same feature as the flows'.
#[cfg(feature = "conformance")]
pub mod spool_conformance;

mod spooled_container;

pub mod sync;

// The folder sync's own contract, behind the same feature as the two ports' and
// the commit flow's.
#[cfg(feature = "conformance")]
pub mod sync_conformance;

mod upload;

// Where the secret-bearing inventory's two promises are asserted, over every
// type on the list and across all three layers it spans (spec: DK-7).
#[cfg(test)]
mod zeroization;
