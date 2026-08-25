//! Putting a run's spooled Containers on Storage, and confirming what arrived.
//!
//! Neither half of it is a fact about which flow produced the ciphertext, so a
//! sync's one-file Containers and a freeze's Packs go up the same way: through
//! the policy's [`RetryPolicy`](crate::RetryPolicy), with the pending row
//! updated as each object lands (spec: OC-2), and against the digest the
//! provider reports for what it stored.
//!
//! It fails in [`UploadError`], which each flow reports under its own names.

mod run;
pub(crate) use run::upload;

mod upload_error;
pub(crate) use upload_error::UploadError;
