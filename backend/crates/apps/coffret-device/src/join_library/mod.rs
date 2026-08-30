//! Taking up a Library another device already created.
//!
//! The same directory [`create_library`](crate::create_library) leaves behind,
//! built from a Recovery Code rather than from an entropy source: the Master Key
//! is entered instead of drawn, its epoch is the code's own, and the Library's
//! place on Storage is stated rather than made. Nothing is written to Storage at
//! all — the app folder, the Keyring and the Journal are already the Library's —
//! so a join that is interrupted leaves nothing anywhere, and the catalog it
//! creates is empty until the first sync or fetch catches it up from the
//! Library's head (spec: CK-9, RV-1).
//!
//! It is what makes the round trip a round trip. Until a second device holds the
//! Library, every Entry has exactly one local copy — the one the sync uploaded
//! from — and a fetch has nowhere to fetch to.

mod join_library_request;
pub use join_library_request::{JoinLibraryRequest, JoinedProvider};

mod joined_library;
pub use joined_library::JoinedLibrary;

mod run;
pub use run::join_library;

#[cfg(test)]
mod tests;
