//! Bringing a Library into being on this device.
//!
//! The steps run in a fixed order, and the order is the design: the Master Key
//! is drawn and stored first because the envelope that opens the account's
//! grant is sealed under a key derived from it, and the settings file is written
//! after the rest because a directory carrying one is a Library anything may
//! open. Everything is built in a staging directory that only takes the
//! Library's real name once the last step has landed, so an interrupted
//! creation leaves something a later attempt can discard rather than half a
//! Library nothing can tell from a whole one.
//!
//! Two things are written to Storage: the app folder a Drive Library needs to
//! exist at all, and, last, the Library's first head — `head-0`, a Journal
//! record with nothing in it, selecting an empty Keyring generation, and
//! carrying the slot the first commit consumes (spec: CP-2, FM-13). Writing it
//! here, before any other device can know of the Library, is what keeps a
//! commit from ever starting from no head, where each writer would reserve a
//! slot of its own. It is the last step because it is the one this flow cannot
//! take back: a failure there leaves what was written under a Library ID that
//! nothing references, and a second attempt draws another.

mod create_library_request;
pub use create_library_request::{CreateLibraryRequest, NewProvider};

mod created_library;
pub use created_library::CreatedLibrary;

mod run;
pub use run::create_library;
#[cfg(test)]
pub(crate) use run::create_library_through;

#[cfg(test)]
mod tests;
