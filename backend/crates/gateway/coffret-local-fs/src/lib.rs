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
//! What the flows promise about local files are promises about *failure*, which
//! [`Spool`](coffret_usecase::Spool) states (spec: OC-2, OC-6), and a filesystem
//! that cannot be asked to refuse a chosen step leaves all of them untested.
//! Naming the operations makes them scriptable — against
//! [`InMemoryFs`](coffret_usecase::InMemoryFs) in a test, against [`UnixFs`]
//! here — and the shared suite behind the use-case crate's `conformance` feature
//! is what keeps the two answering alike.
//!
//! So this crate is the one place the operating system's filesystem API is
//! called on behalf of the flows, and it holds nothing else: no decision about
//! where a Library's spool directory is (that is the composition root's), and no
//! knowledge of what the bytes passing through it are.
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

mod unix_fs;
pub use unix_fs::UnixFs;

mod unix_spool_writer;
