//! Taking up a Library another device already created.
//!
//! The same directory [`create_library`](crate::create_library) leaves behind,
//! built from a Recovery Code rather than from an entropy source: the Master Key
//! is entered instead of drawn, its epoch is the code's own, and the Library's
//! place on Storage is stated rather than made. Nothing is written to Storage at
//! all — the app folder, the Keyring and the Journal are already the Library's —
//! so a join that is interrupted leaves nothing anywhere. The catalog it creates
//! is caught up to the Library's head as the join ends, which is where the
//! Library's name is read from (spec: CK-9, FM-15); a catch-up that does not
//! complete leaves the catalog for the first open to bring up instead.
//!
//! It is what makes the round trip a round trip. Until a second device holds the
//! Library, every Entry has exactly one local copy — the one the sync uploaded
//! from — and a fetch has nowhere to fetch to.

mod found_on_storage;
pub use found_on_storage::FoundOnStorage;

mod join_library_request;
pub use join_library_request::{JoinLibraryRequest, JoinedProvider};

mod joined_library;
pub use joined_library::JoinedLibrary;

// The Library's name, read from the catalog a join catches up as it ends.
mod name_from_catalog;

mod run;
pub use run::join_library;
#[cfg(test)]
pub(crate) use run::join_library_through;

#[cfg(test)]
mod tests;
