//! How a coffret process starts.
//!
//! Two things happen before any binary over [`coffret_device`] does what it was
//! run for: the events it is about to emit are pointed at a file, and — for
//! anything that opens a Library — a Passphrase is asked for. Both are the same
//! in every binary, and neither belongs to any of them.
//!
//! They were the command line's until there was a second binary. Two copies
//! would drift the first time either changed, and what they would drift about is
//! not cosmetic: where a run's evidence is kept, and how a Passphrase is read
//! from a terminal without echoing it or leaving it in a shell history.
//!
//! It is above the domain rather than in it, for the reason the binaries are.
//! [`logging`] installs a subscriber, which only whatever *builds* an
//! application may do — a library crate emits and never installs. [`passphrase`]
//! reads a terminal, which is the shell's half of the callback
//! [`open_library`](coffret_device::open_library) takes: the device layer calls
//! it only once every refusal that needs no key has passed, so a Library that is
//! not on this device costs nobody a prompt.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod logging;

pub mod passphrase;
