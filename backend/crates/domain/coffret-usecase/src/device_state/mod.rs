//! What only this device knows about the Library it catalogs.
//!
//! The [`Index`](crate::Index) holds two kinds of state side by side. One is
//! the catalog of the whole Library, identical on every enrolled device and
//! carried by an Index Snapshot. The other is this: how this device maps the
//! Library onto its own folders (spec: EP-9), which Entries it has actually put
//! on disk (spec: EP-10), and what it has spooled and uploaded but not yet
//! committed (spec: OC-2).
//!
//! None of it is ever uploaded, and a restore leaves all of it untouched
//! (spec: CK-7). That separation is the point: a laptop mapping `albums/` and a
//! desktop mapping `books/` restore the same catalog from one Snapshot and keep
//! their own answers to "where is it on disk" and "did I ever have it".

mod batch_id;
pub use batch_id::BatchId;

mod device_time;
pub use device_time::DeviceTime;

mod local_entry;
pub use local_entry::LocalEntry;

mod local_entry_state;
pub use local_entry_state::LocalEntryState;

mod local_observation;
pub use local_observation::LocalObservation;

mod mapping;
pub use mapping::Mapping;

mod pending_upload;
pub use pending_upload::PendingUpload;
