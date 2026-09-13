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
//! states the first (spec: OC-2, OC-8),
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
//! crate's alone on the way down: no layer above it repeats the reading. The
//! device's registration of a mapped root is beside that line rather than above
//! it — it opens the same reserved names before any descent exists — and it has
//! to read them the same way, which is why *Porting to another Unix* below
//! settles both at once. The configured root itself is deliberately resolved as
//! the user named it; every component below the opened root is
//! descriptor-relative.
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
//!
//! # Porting to another Unix
//!
//! Four questions a port has to answer, because each of them changes what this
//! crate does. Linux and macOS have answers; a build for anything else stops at
//! the `compile_error!` below rather than running on a guess.
//!
//! **What errno a reserved name reports.** Opening a symbolic link with
//! `O_RDONLY | O_NOFOLLOW` reports `ELOOP` on both Linux and macOS; adding
//! `O_DIRECTORY` leaves Linux at `ELOOP` and turns macOS's answer into
//! `ENOTDIR`. Those two are the whole of what this crate reads, and reading is
//! what separates "something else is standing at the reserved name" from "the
//! disk went wrong" (spec: EP-13, EP-14). FreeBSD's manual documents `EMLINK`
//! for the link and NetBSD's `EFTYPE` — quoted rather than measured, and neither
//! is read here, which is exactly the trap: a port whose kernel answers an errno
//! this code does not read would report a local I/O failure where the rule names
//! a verdict. A port measures what its own kernel reports, on a host of that
//! platform, and adds that errno to every place that reads them, which is more
//! than one place: this crate's `refusal`, which the whole descent goes through;
//! the root vouching beside it, which matches the errnos itself rather than
//! through `refusal`; and the device's `root_marker` opens, where a registration
//! meets the same reserved names. All of them together, because a registration
//! and a placement that disagreed would refuse in different vocabularies — and a
//! port that changed `refusal` alone would leave the vouching calling a local
//! I/O failure on the very name the descent beside it had just refused by the
//! rule.
//!
//! **Whether the volume folds case.** The reserved name is compared exactly
//! (spec: EP-14), so on a volume that folds case — macOS's APFS does by
//! default — the name and a case variant of it are the same directory on disk,
//! and only the exact spelling is recognized here as the reserved one. This is
//! not settled on the platforms already here: a port inherits the hole rather
//! than a working rule.
//!
//! **Whether a lookup ignores how a name is composed.** A scan does not rest
//! on that: composing a name to NFC decides its Library position (spec: EP-1),
//! while the validated relative location keeps the spelling the directory
//! actually handed back, and that is the one a later source read reopens
//! (spec: EP-8). A fetch is the side with only the composed spelling to place
//! at, so a volume that stores a name in another one *and* tells the two apart
//! in lookup puts a fetched Entry beside the file it belongs at rather than at
//! it. APFS folds the difference away in lookup; a port asks the volumes it
//! will run on.
//!
//! **Whether `st_birthtime` exists.** Answered per platform already, by the
//! listing this crate's mapped roots are read through; named here so the list is
//! the whole of what a port settles rather than most of it.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

// The reading above is what a platform has to answer for before a binary is
// built for it, and one gate serves every crate: the device's own marker opens
// read the same errnos with their own `rustix` calls, but nothing that reads
// them reaches a compiler without depending on this crate. It also replaces what
// a Windows build produces — a pile of type errors out of `rustix` — with one
// sentence. `target_vendor = "apple"` is the spelling the mapped roots' listing
// already uses for the birth-time split.
#[cfg(not(any(target_os = "linux", target_vendor = "apple")))]
compile_error!(
    "coffret's local filesystem gateway reads the errno a reserved name reports, \
     and only Linux and macOS have been measured: ELOOP for a symbolic link, and \
     ENOTDIR once O_DIRECTORY is passed beside O_NOFOLLOW. Porting to another \
     Unix means measuring its own on a host of that kind and reading it here — \
     FreeBSD documents EMLINK for the link and NetBSD EFTYPE, and neither is \
     read. See this crate's documentation for the rest of what a port settles."
);

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
