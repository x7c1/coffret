//! What the examples in this directory share.
//!
//! A directory rather than `examples/support.rs`, because cargo compiles every
//! `.rs` file directly under `examples/` as an example of its own and this is
//! not one. A directory without a `main.rs` is passed over, while `mod
//! support;` in either example still finds the `mod.rs` inside it.

use std::error::Error;

/// `error` and every link beneath it, joined the way the binaries print a
/// chain.
///
/// The examples print concrete error types rather than an `anyhow::Error`, so
/// the `{error:#}` the binaries end on does nothing for them: the `#` flag is
/// anyhow's own, and a concrete `Display` ignores it. Each wrapper here says
/// only what its own layer knows and leaves the rest to `source`, so a line
/// that renders the outermost sentence alone stops at "could not listen for
/// the redirect" and drops the "Address already in use" that says why. This
/// walks the links so the operator reading a terminal gets both.
///
/// Unlike `chain_to_the_workspace_edge` in this crate's `error/into_port.rs`, the walk
/// does not stop where this workspace's own error types end. That one is
/// building a `detail` field bound by a
/// redaction contract — a foreign library hangs the configured host off links
/// of its own, and none of that may reach a diagnostic event. Nothing of the
/// sort binds standard error on a machine whose operator asked for this run,
/// and the foreign library's own links are frequently the only ones that name
/// the actual failure, so the whole chain goes out.
pub fn every_link(error: &dyn Error) -> String {
    let mut rendered = error.to_string();
    let mut below = error.source();
    while let Some(link) = below {
        rendered.push_str(": ");
        rendered.push_str(&link.to_string());
        below = link.source();
    }
    rendered
}
