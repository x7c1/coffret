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
//! # One JSON object per line
//!
//! The file is JSONL. Every event is one object on one line, and the fields it
//! was emitted with are fields:
//!
//! ```text
//! {"timestamp":"2026-08-21T03:49:39.523882Z","level":"WARN","fields":{"message":"Storage refused access","operation":"put","status":403,"reason":"AccessDenied","body":"<Error><Code>AccessDenied</Code>…"},"target":"s3_store::error"}
//! ```
//!
//! That follows from what the file is for. The questions it has to answer are
//! aggregate ones — every catch-all `warn` grouped by the reason a provider
//! gave, whether an unfamiliar 403 has ever arrived, how often a retry gave up
//! — and they are questions about fields, which `jq` answers from the record
//! rather than from a regular expression over a message line:
//!
//! ```sh
//! cd "${XDG_STATE_HOME:-$HOME/.local/state}/coffret/logs"
//!
//! # every reason a provider refused with, and how often each arrived
//! jq -R 'fromjson? // empty | select(.level == "WARN") | .fields.reason' coffret-*.log |
//!   sort | uniq -c
//!
//! # what one operation did, in the order it did it
//! jq -R -c 'fromjson? // empty | select(.fields.operation == "put")' coffret-*.log
//! ```
//!
//! `fromjson? // empty` is what makes a reader safe against the one line that
//! may not be JSON. A record too large for a file is cut rather than dropped,
//! and half an object is not one; the cut line is followed by a marker record
//! carrying `"truncated": true`, which does parse, so a query sees that a
//! record was lost there instead of a parse error with nothing behind it. Plain
//! `jq .` stops at the cut line, and `jq -R 'fromjson? // empty'` steps over it
//! and keeps going.
//!
//! Nothing is in a record but the timestamp, the level, the target, and the
//! event's own fields: no source file, no line number, no span, no thread. That
//! is deliberate — see the sink in `install` — and it is what keeps the rule
//! below about what may never be written a rule about events alone.
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
//! Naming the fields costs bytes, and the ceiling is what pays. One run of the
//! S3 conformance suite against MinIO (`make s3-store-it`) emits 80 events, and
//! they weigh:
//!
//! | format | bytes | per event |
//! | --- | --- | --- |
//! | the human-readable one this replaced | 11,628 | 145 |
//! | JSONL | 16,793 | 210 |
//!
//! A factor of 1.44, so the same ceiling holds around seven runs' worth of
//! evidence where it used to hold ten. That is the trade, stated rather than
//! assumed: a question like "every reason a provider refused with, counted" is
//! answerable from a record and guesswork from a message line, and it is paid
//! for in how far back the answers go. Raise `COFFRET_LOG_MAX_BYTES` if the
//! history matters more than the disk.
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

mod jsonl;

mod log_settings;
pub use log_settings::{default_directory, LogSettings, LOG_DIRECTORY, LOG_LEVEL, LOG_MAX_BYTES};

pub mod redact;

mod rotating_files;
pub use rotating_files::RotatingFiles;

#[cfg(feature = "testing")]
pub mod testing;
