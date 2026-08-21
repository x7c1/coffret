//! Reading back what a piece of code emitted.
//!
//! Every rule about logging in coffret is a rule about what an event carries —
//! that a refusal the port has no state for is recorded at all, that a missing
//! object is not an error, that no credential ever reaches the file. None of
//! that is provable by reading the code that calls `warn!`; it is provable by
//! running the path and looking at what came out, which is what this is for.
//!
//! The subscriber it installs is the *thread's* default rather than the
//! process's, so cases capture their own events without interfering with each
//! other and without a global install that would outlive the test that wanted
//! it.

//! What is read back is JSONL, the same as what the sink writes, so a case can
//! ask an event for the field it cares about rather than for a substring of a
//! line.

mod capture_writer;

mod captured_logs;
pub use captured_logs::CapturedLogs;

mod logged_event;
pub use logged_event::LoggedEvent;
