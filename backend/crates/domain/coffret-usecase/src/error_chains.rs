//! The one place inside this crate where its own tests and its conformance
//! suites render an error and everything beneath it as one line.
//!
//! Every suite panics with what a failure said, and what a failure said is a
//! chain: each wrapper names its own layer and leaves the rest to `source`.
//! The walk lives here rather than at each of the suites so that the sentence
//! a panic carries is one sentence everywhere, instead of as many spellings as
//! there are suites to drift apart.
//!
//! Inside this crate, and no further: a test target under `tests/` is compiled
//! against `coffret_usecase` as any other dependent is, so it cannot name a
//! module the crate keeps to itself. `tests/scan_faults.rs` therefore holds a
//! copy of this walk, and says there why it is one rather than a share of this.

/// `error` and every link beneath it, joined the way a caller printing
/// `{error:#}` reads them.
///
/// Every wrapper a suite meets — `coffret_usecase::Error`,
/// `coffret_format::Error`, `CommitError` — names only its own layer in
/// `Display` and leaves what the layer below answered to `source`, so a bare
/// `{error}` in a panic drops everything under the top line, which is usually
/// the part that says what actually went wrong.
///
/// Named apart from the `chain` each error type's own cases keep: that one
/// hands the links back one at a time, to be asserted over, and this one
/// renders them for somebody reading a panic.
pub(crate) fn every_link(error: &dyn std::error::Error) -> String {
    let mut rendered = error.to_string();
    let mut cause = error.source();
    while let Some(link) = cause {
        rendered.push_str(": ");
        rendered.push_str(&link.to_string());
        cause = link.source();
    }
    rendered
}
