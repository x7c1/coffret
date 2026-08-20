//! Where coffret's events go, and what may be written into them.
//!
//! Coffret depends on third-party Storage APIs whose real behaviour is not
//! fully knowable from their documentation — an upload cap whose refusal is
//! undocumented, minted identifiers with no stated lifetime — so the first
//! purpose of logging here is not debugging. It is to keep evidence of what a
//! provider actually answered, in a form a person can read afterwards and
//! decide from.
//!
//! Two halves, and the split is deliberate:
//!
//! - **The facade.** Every crate that has something worth recording depends on
//!   [`tracing`] and emits events. None of them installs a subscriber, so a
//!   test, or a binary that wants no logging, simply sees nothing emitted and
//!   behaves identically.
//! - **The sink.** [`install`] is called once by whatever *builds* an
//!   application — a binary, an example, a test harness that talks to a live
//!   API — and points the events at a file under the state directory.
//!
//! The crate sits in `libs/` because it is neither the domain nor one
//! provider's gateway: it is the one place the file layout, the size ceiling,
//! and the redaction rule are stated, so that every entry point installs the
//! same sink instead of inventing its own.
//!
//! # A ceiling on bytes, not on files
//!
//! [`RotatingFiles`] rotates on size and prunes the oldest files until what is
//! left fits a total-byte ceiling. `tracing-appender`'s `RollingFileAppender`
//! is the obvious thing to reach for and does not meet the requirement: it
//! rotates by period and its `max_log_files` bounds the *number* of files,
//! which leaves a single busy day free to fill the disk. Nothing here is
//! allowed to grow without bound, so the ceiling is on bytes and the oldest
//! files are what go — recent failures being the evidence worth keeping.
//!
//! # Coffret's own events, and nobody else's
//!
//! That ceiling is why [`install`] keeps the file to coffret's own crates by
//! default. The budget is finite and every target shares it, so a dependency
//! narrating its own internals does not merely make the file harder to read —
//! it *evicts*. A cloud SDK writes hundreds of kilobytes about retries and
//! endpoint resolution in a single run, pruning drops the oldest bytes, and
//! within a few runs the `warn` that recorded an unfamiliar provider answer is
//! gone while the chatter that pushed it out is what remains.
//!
//! `COFFRET_LOG` widens it for somebody who has decided they want a
//! dependency's account of itself — `debug,aws_smithy_runtime` — and is
//! spending the budget on it knowingly. Widening changes only *whose* events
//! are kept. How loud they may be is capped at `DEBUG` separately, so naming a
//! target lets through the same `DEBUG` that coffret's own crates already reach
//! the file with, and never the `TRACE` where an HTTP stack prints its headers
//! and a signer prints its signing material.
//!
//! # What must never be written into an event
//!
//! Coffret hides the user's folder structure from the Storage provider behind
//! opaque object names; writing it into a plaintext log on the same disk would
//! open the exact leak the design closes, outside the reach of whole-disk
//! encryption. So no event may carry an Entry Path or a local file name,
//! plaintext or any fragment of it, any key material or the Passphrase or a
//! Recovery Code, or an OAuth token or the `Authorization` header.
//!
//! Opaque values are safe and useful: object names, Container IDs,
//! generations, ciphertext sizes and hashes, HTTP statuses, provider reason
//! strings. Provider response bodies are safe for the same reason — the names
//! coffret sends a provider are opaque — but a body from an OAuth endpoint
//! could carry a token, so [`redact`] takes credentials out of one and caps its
//! length rather than dropping the event whole.
//!
//! ```no_run
//! # fn main() -> Result<(), coffret_logging::Error> {
//! let settings = coffret_logging::LogSettings::from_env()?;
//! let path = coffret_logging::install(&settings)?;
//! // Printed rather than logged: where the file is, is a local path, and a
//! // local path is one of the things an event may not carry.
//! println!("logging this run to {}", path.display());
//! tracing::info!(operation = "start", "the sink is installed");
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

mod error;
pub use error::{Error, Result};

mod install;
pub use install::install;

mod log_settings;
pub use log_settings::{default_directory, LogSettings, LOG_DIRECTORY, LOG_LEVEL, LOG_MAX_BYTES};

pub mod redact;

mod rotating_files;
pub use rotating_files::RotatingFiles;

#[cfg(feature = "testing")]
pub mod testing;
