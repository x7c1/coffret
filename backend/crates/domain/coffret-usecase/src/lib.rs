//! What the use-case layer asks of the world outside it.
//!
//! Coffret keeps a Library on Storage it does not trust, and the format layer
//! that turns user data into Storage Objects knows nothing about where those
//! objects go. This crate is the seam between them: [`ObjectStore`] is the one
//! port every Storage provider is reached through, and the vocabulary around it
//! — [`ObjectRef`], [`CommitSlot`], [`ObjectInfo`], [`Error`] — is the storage
//! vocabulary the rest of the backend reasons in.
//!
//! The port is deliberately narrow, because Storage is only ever handed
//! ciphertext and only ever asked to keep it, hand it back, enumerate it, and
//! remove it. Two things it does fix, because the Library's correctness rests on
//! them rather than on any one provider:
//!
//! - **Conditional create.** [`ObjectStore::reserve_create`] and
//!   [`ObjectStore::put_if_absent`] are how a commit is won or lost: of several
//!   writers spending one slot exactly one succeeds, and the rest get
//!   [`Error::AlreadyExists`] — a state, not a transport hiccup.
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
//! [`Error::is_retryable`] answers from the type alone.
//!
//! Behind the `conformance` feature, [`conformance`] is the contract as a suite
//! of tests every adapter runs, so a second provider cannot quietly redefine
//! what the port means.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod byte_stream;
pub use byte_stream::ByteStream;

mod commit_slot;
pub use commit_slot::CommitSlot;

#[cfg(feature = "conformance")]
pub mod conformance;

mod error;
pub use error::{Error, Result};

mod object_info;
pub use object_info::ObjectInfo;

mod object_page;
pub use object_page::ObjectPage;

mod object_ref;
pub use object_ref::ObjectRef;

mod object_store;
pub use object_store::ObjectStore;

mod page_token;
pub use page_token::PageToken;

mod provider_hash;
pub use provider_hash::ProviderHash;
