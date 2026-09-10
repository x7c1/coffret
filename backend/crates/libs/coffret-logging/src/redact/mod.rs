//! Taking credentials and private request data out of what an event is about
//! to carry.
//!
//! A provider's response body is useful evidence after private request data and
//! credentials have been removed. Object identifiers are opaque, but request
//! data is not limited to object identifiers: a configured bucket or prefix can
//! identify a person's arrangement, and a provider is free to echo it. OAuth
//! bodies can carry tokens, and any refusal may quote a header that carried
//! one. Those values are cut out rather than the whole event being dropped,
//! because what an endpoint refused with is exactly the evidence worth keeping.
//!
//! A URL is the other thing that arrives already holding a credential — in its
//! query string rather than in a body — which is what [`url()`] is for.
//!
//! Over-redaction is the deliberate direction of error: a value that merely
//! looks like a credential is replaced too. The rule the never-list is drawn
//! from is that nothing which *grants access* may be written to a file, whether
//! or not it is called a token; the file's mode is not what is relied on to
//! keep one safe.
//!
//! A caller-owned location refines that direction rather than reversing it.
//! Such a value is neither a credential nor a shape: it is a string the caller
//! handed in, held in [`PrivateValues`], and it is removed only where it stands
//! as a whole token — where the characters on either side of it are ones a
//! bucket name or a path segment cannot contain (the start or end of the text,
//! whitespace, `/ \ " ' < > ( ) [ ] { } , ; : = ? & % + * | @ ! # $ ^ ~`, a
//! backtick, or an ASCII control character; `-`, `.` and `_` are ordinary
//! inside a location and so bound nothing). That holds at every length, with no
//! minimum and no configuration refused. The alternative to a boundary is a
//! length threshold, and a threshold would leave the location in the log for
//! exactly the people whose bucket is short — S3 allows three characters —
//! while replacing a three-character value wherever it appeared would turn the
//! provider's own words into wreckage — a bucket called `log` leaving
//! `[redacted]ging configuration` behind — and lose the evidence the event
//! exists for.
//!
//! Inside those bounds over-redaction still decides the doubtful cases: every
//! spelling a request could have carried the value in is taken out, and a value
//! that contains another is taken out first so that no fragment is left.
//!
//! Each rule that takes one kind of value out lives in a module of its own, so
//! that adding a rule adds a module rather than a paragraph to an existing one.

mod body;
pub use body::{body, body_without};

mod private_values;
pub use private_values::PrivateValues;

mod text;
pub use text::{text, text_without};

mod url;
pub use url::url;

mod without_bearer;

mod without_field;

mod without_private;

/// The most of one body an event carries.
///
/// A body is evidence, not a copy of Storage: a refusal that runs to pages says
/// what it means in its first lines, and letting one event fill a whole log
/// file would push out the events around it that give it context.
pub const MAX_BODY_BYTES: usize = 2048;

/// What is left in place of a credential.
const REDACTED: &str = "[redacted]";
