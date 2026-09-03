//! Rendering a failure from below into something this server's log may carry.
//!
//! # Why `Display` is not it
//!
//! Every refusal these routes answer with keeps what the layer below reported,
//! and the log is the one place it goes (see [`ApiError`](super::ApiError)).
//! What it used to go there as was the whole cause chain rendered with
//! `Display`, on the reasoning that a rendering of an error is not a rendering
//! of the request. That reasoning does not hold, and the vocabularies below say
//! so in their own doc comments: a fetch's refusals are *identified* by an
//! Entry Path — no folder here holds this part of the Library, this device
//! cannot spell a name for that path, two paths would land on one file — and
//! the message names it because the message is written for the person who owns
//! the Library and is standing in front of it. Why none of that may reach a log
//! file is [`Redacted`](coffret_device::Redacted)'s own doc.
//!
//! # What replaces it
//!
//! [`Redacted`](coffret_device::Redacted), which every vocabulary a refusal can
//! come from implements beside its `Display`: each error in a chain contributes
//! its identity and the facts a log may carry, and its cause contributes the
//! same underneath. The trait rather than an allowlist here, because the choice
//! of what may be said about a variant belongs with the variant — a fetch
//! knowing that its `component` is a local folder is the same knowledge that
//! made it name one — and because a chain walked as `dyn Error` cannot tell a
//! caller which type each link is, so an allowlist would render everything it
//! did not recognise as nothing at all.
//!
//! Two leaves are not coffret's and stay here, which is the allowlist half:
//! [`multipart`] and [`io_failure`], for the extractor's refusal and the
//! operating system's. Both are foreign types under a foreign trait, so neither
//! could implement `Redacted` even if it were this crate's business to say what
//! `axum` may write down.
//!
//! The rendering is enforced by construction rather than by care at each site:
//! a refusal holds the redacted text and never the failure, so there is nothing
//! left for a later `error = %cause` to reach for.

mod io_failure;
pub(crate) use io_failure::io_failure;

mod multipart;
pub(crate) use multipart::multipart;
