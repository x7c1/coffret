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
//! Entry Path — "no folder on this device holds this part of the Library", this
//! device cannot spell a name for that path, two paths would land on one file —
//! and the message names it because the message is the response to the person
//! whose request was refused, and which path it was is what they asked to be
//! told. Why none of that may reach a log file is [`Redacted`](coffret_device::Redacted)'s own doc.
//!
//! # What replaces it
//!
//! [`Redacted`](coffret_device::Redacted), which every vocabulary a refusal can
//! come from implements beside its `Display`: each error in a chain contributes
//! its identity and the facts a log may carry, and its cause contributes the
//! same underneath. The trait rather than an allowlist here, because the choice
//! of what may be said about a variant belongs with the variant — a fetch
//! knowing that its `stopped_at` is a local folder is the same knowledge that
//! made it name one — and because a chain walked as `dyn Error` cannot tell a
//! caller which type each link is, so an allowlist would render everything it
//! did not recognise as nothing at all.
//!
//! The leaves that are not coffret's stay here, which is the allowlist half:
//! [`Multipart`], [`Json`] and [`IoFailure`], for the extractors' refusals and
//! the operating system's. Each is a foreign type under a foreign trait, so
//! none could implement `Redacted` even if it were this crate's business to say
//! what `axum` may write down; each is wrapped here instead, as the value it is,
//! by a type that says what of it an event may carry. [`NotJoined`] is the same
//! for the runtime's account of a blocking task that ended without an answer,
//! and [`NotHeld`] and [`Named`] for a refusal of the drop store's that has no
//! rendering of its own and for a state with nothing below it.
//!
//! The rendering is enforced by construction rather than by care at each site.
//! A refusal keeps the failure as the value the layer below reported, but holds
//! it as a `dyn Redacted` and as nothing else: neither `Display` nor `Debug` is
//! reachable through it, so the one rendering a later `error = …` can reach for
//! is this one, and it is made only where the refusal is logged.

mod io_failure;
pub(crate) use io_failure::IoFailure;

mod json;
pub(crate) use json::Json;

mod multipart;
pub(crate) use multipart::Multipart;

mod named;
pub(crate) use named::Named;

mod not_held;
pub(crate) use not_held::NotHeld;

mod not_joined;
pub(crate) use not_joined::NotJoined;
