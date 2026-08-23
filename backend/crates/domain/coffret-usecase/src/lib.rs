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
//! Behind the `conformance` feature, [`conformance`] and
//! [`index_conformance`](mod@index_conformance) are the two contracts as suites
//! of tests every adapter runs, so a second adapter cannot quietly redefine
//! what a port means. [`InMemoryStore`] and [`InMemoryIndex`] are what to drive
//! them — and the crate's own cases — against without a provider, a container,
//! or a file. This crate runs both suites against those two.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod byte_stream;
pub use byte_stream::ByteStream;

mod commit_slot;
pub use commit_slot::CommitSlot;

mod committed_batch;
pub use committed_batch::CommittedBatch;

#[cfg(feature = "conformance")]
pub mod conformance;

mod control_head;
pub use control_head::ControlHead;

pub mod device_state;

mod error;
pub use error::{Error, Result};

mod index;
pub use index::Index;

// The `Index` contract as a suite, behind the same feature as the storage
// port's, and for the same reason: only a test target needs it.
#[cfg(feature = "conformance")]
pub mod index_conformance;

mod index_error;
pub use index_error::{IndexError, IndexResult};

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
