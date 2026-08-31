//! Turning what a device recorded about a Library into the things a flow runs
//! on.
//!
//! A sync, a freeze, and a fetch each take a store, a catalog, the keys of one
//! Master Key epoch, and somewhere to spool. None of them knows which provider
//! the Library is on, and none of them should: this module is the one place the
//! settings file's answer becomes a concrete gateway.

use std::path::PathBuf;
use std::sync::Arc;

use coffret_model::{LibraryId, MasterKeyEpoch};
use coffret_usecase::{Index, LibraryKeys, ObjectStore};

mod run;
pub use run::open_library;

mod store;

/// One Library, open on this device.
///
/// The unlocked Master Key is not among the fields, and that is deliberate:
/// what the flows need from it is [`LibraryKeys`], which is derived once here,
/// so the key itself has one owner and one lifetime rather than being carried
/// through every call that might want a key derived from it (spec: DK-9).
pub struct OpenLibrary {
    /// The Library's Storage, whichever provider it is on.
    pub store: Arc<dyn ObjectStore>,
    /// The device-local catalog of this Library.
    pub index: Arc<dyn Index>,
    /// What this Master Key epoch's Containers are sealed and opened with.
    pub keys: LibraryKeys,
    /// Where encrypted Containers wait until they are uploaded.
    pub spool: PathBuf,
    /// The Library this is (spec: FM-18).
    pub library_id: LibraryId,
    /// The Master Key epoch [`keys`](Self::keys) belongs to.
    pub epoch: MasterKeyEpoch,
    /// Which provider the Library's Storage is, in the settings file's own word.
    ///
    /// The one thing about where a Library lives that a shell may show without
    /// reading the settings for itself: it names the provider and nothing about
    /// the account, the bucket, the folder, or the grant. It is carried here so
    /// that opening a Library reads those settings once.
    pub provider: &'static str,
}
