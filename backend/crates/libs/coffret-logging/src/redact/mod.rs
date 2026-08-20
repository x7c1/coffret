//! Taking credentials out of what an event is about to carry.
//!
//! A provider's response body is worth recording verbatim: the names coffret
//! sends a provider are opaque, so a body says what really came back without
//! saying anything about the Library. One family of bodies is different — the
//! OAuth endpoint's, which carry tokens — and one refusal from a provider may
//! quote back a header that carried one. Those are cut out here rather than the
//! whole event being dropped, because what an endpoint refused with is exactly
//! the evidence worth keeping.
//!
//! A URL is the other thing that arrives already holding a credential — in its
//! query string rather than in a body — which is what [`url`] is for.
//!
//! Over-redaction is the deliberate direction of error: a value that merely
//! looks like a credential is replaced too. The rule the never-list is drawn
//! from is that nothing which *grants access* may be written to a file, whether
//! or not it is called a token; the file's mode is not what is relied on to
//! keep one safe.
//!
//! Each rule that takes one kind of credential out lives in a module of its
//! own, so that adding a rule adds a module rather than a paragraph to an
//! existing one.

mod body;
pub use body::body;

mod text;
pub use text::text;

mod url;
pub use url::url;

mod without_bearer;

mod without_field;

/// The most of one body an event carries.
///
/// A body is evidence, not a copy of Storage: a refusal that runs to pages says
/// what it means in its first lines, and letting one event fill a whole log
/// file would push out the events around it that give it context.
pub const MAX_BODY_BYTES: usize = 2048;

/// What is left in place of a credential.
const REDACTED: &str = "[redacted]";
