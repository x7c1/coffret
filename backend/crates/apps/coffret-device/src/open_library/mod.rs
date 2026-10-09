//! Turning what a device recorded about a Library into the things a flow runs
//! on.
//!
//! A sync, a freeze, and a fetch each take a store, a catalog, and the keys of
//! one Master Key epoch, and each takes this device's own disk as well: the two
//! that scan want the spool to write into and the mapped folders to read, and
//! the fetch wants the folders it places into. None of them knows which
//! provider the Library is on or whose filesystem it is reading, spooling onto,
//! and placing into, and none of them should: this module is the one place the
//! settings file's answer — and the device's own disk — becomes a concrete
//! gateway.

use std::path::PathBuf;
use std::sync::Arc;

use coffret_format::ParcelLen;
use coffret_local_fs::UnixFs;
use coffret_model::{LibraryId, MasterKeyEpoch};
use coffret_usecase::{Index, LibraryKeys, ObjectStore, UnknownBirths};

use crate::drive_grant::DriveGrant;

mod run;
pub use run::open_library;
#[cfg(test)]
pub(crate) use run::open_library_through;

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
    /// This device's own disk, as the three flows that read and write it ask
    /// for it: the spool, the mapped folders, and the places a local writer
    /// puts a file.
    ///
    /// One per open Library rather than one per run, because it holds nothing:
    /// every call names the path it is about. It is the concrete gateway here
    /// and a capability everywhere above, which is what lets a flow be driven
    /// over a disk that refuses on request.
    pub local_fs: Arc<UnixFs>,
    /// What this Master Key epoch's Containers are sealed and opened with.
    pub keys: LibraryKeys,
    /// Where encrypted Containers wait until they are uploaded.
    pub spool: PathBuf,
    /// Where the parcels a fetch read are kept (spec: PK-21).
    pub parcel_dir: PathBuf,
    /// How long a parcel is: the register's value, provisionally 32 MiB
    /// (spec: PK-19).
    ///
    /// A field rather than the constant read where it is used, so that a test
    /// driving a whole server can have a Pack of a few megabytes be several
    /// parcels long.
    pub parcel_len: ParcelLen,
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
    /// The grant [`store`](Self::store) reaches Drive through, and `None` for
    /// a Library on any other provider.
    ///
    /// Held so that a process that stays up can renew an expired grant without
    /// the Passphrase it has already spent; [`DriveGrant`] says why holding it
    /// adds nothing a lock has to end.
    pub grant: Option<DriveGrant>,
    /// The files this device placed into a mapped folder with no birth time of
    /// their own — what [`add_file`](Self::add_file) writes — so that the sync
    /// or freeze that carries them in records none for them (spec: FM-9,
    /// EP-11).
    ///
    /// Held here, with the open Library, because the scan it is for is the one
    /// the drop arms a moment later; [`UnknownBirths`] says what is forgotten
    /// with it.
    pub births: Arc<UnknownBirths>,
}
