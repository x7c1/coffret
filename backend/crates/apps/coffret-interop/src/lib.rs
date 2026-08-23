//! The Rust half of the cross-implementation fixture exchange.
//!
//! coffret carries two independent implementations of the storage format: the
//! Rust crates under `crates/domain`, and `@coffret/format` in TypeScript. Each
//! was written from the published specification rather than from the other, so
//! agreement between them is evidence about the specification and not about a
//! shared codebase. This crate turns that agreement into something a pipeline
//! can fail on:
//!
//! - [`generate()`] writes a fixture set — objects plus a `manifest.json` stating
//!   the key material to open them and the values they must decode to — for the
//!   TypeScript side to read.
//! - [`verify()`] opens the set the TypeScript side wrote in return and checks
//!   every fixture against that same manifest schema.
//!
//! Both directions run in one pass, so a disagreement is caught whichever
//! implementation writes and whichever reads. When the exchange fails, either
//! the specification or one implementation is wrong; the fix is a deliberate
//! change to whichever it is, never a loosened check here.
//!
//! The format crates stay byte-in, byte-out: all file access lives here.

#![forbid(unsafe_code)]

mod fixture_set;
mod hex;

pub mod manifest;

mod generate;
pub use generate::generate;

mod verify;
pub use verify::verify;
