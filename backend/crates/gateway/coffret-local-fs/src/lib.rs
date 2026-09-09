//! This device's own disk, reached through the capabilities the use-case layer
//! names.
//!
//! The two ports — [`ObjectStore`](coffret_usecase::ObjectStore) and
//! [`Index`](coffret_usecase::Index) — exist because Storage and a catalog are
//! things a Library could be kept on many of. The local filesystem is not one of
//! those: there is one disk under a device, and nothing about it is behind the
//! trust boundary the ports cross. It is reached through capabilities all the
//! same, for a different reason.
//!
//! What the flows promise about local files are promises about *failure*, about
//! *absence*, and about *interruption* — [`Spool`](coffret_usecase::Spool)
//! states the first (spec: OC-2, OC-6),
//! [`MappedRoots`](coffret_usecase::MappedRoots) the second (spec: EP-8, EP-12),
//! and [`Destinations`](coffret_usecase::Destinations) the third (spec: EP-4,
//! EP-11) — and a filesystem that cannot be asked to refuse a chosen step, to
//! lose a folder between two calls, or to fail the rename that publishes a
//! verified file, leaves all of them untested. Naming the operations makes them
//! scriptable — against the use-case crate's `InMemoryFs` in a test,
//! against [`UnixFs`] here — and the shared suites behind the use-case crate's
//! `conformance` feature are what keep the two answering alike.
//!
//! So this crate is the one place the operating system's filesystem API is
//! called on behalf of the flows, and it holds nothing else: no decision about
//! where a Library's spool directory is (that is the composition root's), no
//! knowledge of what the bytes passing through it are, no reading of what a name
//! in a mapped folder means for the Library — turning one into an Entry Path is
//! the walk's, above this line (spec: EP-1) — and no say in whether a file may
//! be placed at a path, which is the fetch's (spec: EP-10, EP-11). What it does
//! decide, and nothing above it may, is what an errno means: a symbolic link
//! below a mapped root is neither a source this device may read nor a path it
//! may materialize through. Reading `ELOOP` and `ENOTDIR` to know that is this
//! crate's alone. The configured root itself is deliberately resolved as the
//! user named it; every component below the opened root is descriptor-relative.
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
// into being, as the values an Entry carries (spec: FM-9), and the way back for
// the fetch that stamps a file it placed with the time its Entry records
// (spec: EP-11). Here rather than in the use-case crate because both directions
// mean holding the operating system's own form of a moment, which is this
// crate's alone to hold.
mod local_times;

mod unix_fs;
pub use unix_fs::UnixFs;

// The two other capabilities `UnixFs` answers, the `Spool` being in `unix_fs.rs`
// with the type itself: the mapped folders a scan reads, and the places a fetch
// writes into. Each is a directory of its own because its descriptor-relative
// operations divide into several independent responsibilities.
mod unix_destinations;

mod unix_mapped_roots;

// And the handles the three hand out: the reader a mapped file is read through,
// the writer a spool file is written through, and — beside the descent, in
// `unix_destinations` — the scratch file a placement is written to and the
// flushed file it is published from.
mod unix_source_reader;

mod unix_spool_writer;
