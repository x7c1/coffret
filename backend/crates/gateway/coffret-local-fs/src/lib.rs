//! This device's own disk, reached through the capabilities the use-case layer
//! names.
//!
//! The two ports — [`ObjectStore`](coffret_usecase::ObjectStore) and
//! [`Index`](coffret_usecase::Index) — exist because Storage and a catalog are
//! things a Library could be kept on many of. The local filesystem is not one of
//! those: there is one disk under a device, and nothing about it is behind the
//! trust boundary the ports cross. It is reached through a capability all the
//! same, for a different reason.
//!
//! What the flows promise about local files are promises about *failure* and
//! about *absence* — [`Spool`](coffret_usecase::Spool) states the first
//! (spec: OC-2, OC-6) and [`MappedRoots`](coffret_usecase::MappedRoots) the
//! second (spec: EP-8, EP-12) — and a filesystem that cannot be asked to refuse
//! a chosen step, or to lose a folder between two calls, leaves all of them
//! untested. Naming the operations makes them scriptable — against
//! [`InMemoryFs`](coffret_usecase::InMemoryFs) in a test, against [`UnixFs`]
//! here — and the shared suites behind the use-case crate's `conformance`
//! feature are what keep the two answering alike.
//!
//! So this crate is the one place the operating system's filesystem API is
//! called on behalf of the flows, and it holds nothing else: no decision about
//! where a Library's spool directory is (that is the composition root's), no
//! knowledge of what the bytes passing through it are, and no reading of what a
//! name in a mapped folder means for the Library — turning one into an Entry
//! Path is the walk's, above this line (spec: EP-1).
//!
//! ```no_run
//! use std::path::Path;
//!
//! use coffret_local_fs::UnixFs;
//! use coffret_usecase::{LocalIoError, Spool};
//!
//! # async fn example() -> Result<(), LocalIoError> {
//! let local = UnixFs::new();
//! local.prepare_dir(Path::new("/var/lib/coffret/spool")).await?;
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![warn(missing_docs)]

// What a filesystem says about when a file was last changed and when it came
// into being, as the values an Entry carries (spec: FM-9). Here rather than in
// the use-case crate because reading them means holding the operating system's
// own metadata, which is this crate's alone to hold.
mod local_times;

mod unix_fs;
pub use unix_fs::UnixFs;

// The mapped-roots capability `UnixFs` answers, the `Spool` being in
// `unix_fs.rs` with the type itself, and the handles the two hand out: the
// reader a mapped file is read through, and the writer a spool file is written
// through.
mod unix_mapped_roots;

mod unix_source_reader;

mod unix_spool_writer;
