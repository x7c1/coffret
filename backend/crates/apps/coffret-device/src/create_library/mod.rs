//! Bringing a Library into being on this device.
//!
//! Seven steps in a fixed order, and the order is the design: the Master Key is
//! drawn and stored first because the OAuth cache is sealed under a key derived
//! from it, and the settings file is written last because a directory carrying
//! one is a Library anything may open. Everything is built in a staging
//! directory that only takes the Library's real name once the last step has
//! landed, so an interrupted creation leaves something a later attempt can
//! discard rather than half a Library nothing can tell from a whole one.
//!
//! Nothing is written to Storage but the app folder a Drive Library needs to
//! exist at all. Keyring generation 1 and Journal record 1 are the first
//! commit's work, which is what makes a Library that has never been synced
//! indistinguishable on Storage from one that was never created — and what lets
//! this flow be abandoned at any point without leaving a Library behind.

mod create_library_request;
pub use create_library_request::{CreateLibraryRequest, NewProvider};

mod created_library;
pub use created_library::CreatedLibrary;

mod run;
pub use run::create_library;

#[cfg(test)]
mod tests;
