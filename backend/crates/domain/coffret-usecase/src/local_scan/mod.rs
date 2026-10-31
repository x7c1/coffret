//! Reading this device's mapped folders off its own disk.
//!
//! Where a file stands in the Library is decided by the mappings and nothing
//! else (spec: EP-9), and which local files are Entries at all is decided by
//! EP-8 and EP-11. Both answers are the same whichever flow is asking, so the
//! walk is written once here: [`sync`](crate::sync) carries what it finds into
//! the Library one file at a time, and [`freeze`](crate::freeze) packs what it
//! finds into Packs, and neither may disagree with the other about what is
//! there.
//!
//! Whether a mapped root may be read as evidence at all is decided here too,
//! and before anything under it is walked (spec: EP-12): a root that is not
//! there, and one that is empty because the filesystem it stood on is no longer
//! mounted, are reported as unavailable rather than read as folders holding
//! nothing. Both flows carry that verdict into their own outcome, because both
//! would otherwise draw a conclusion from an absence they cannot vouch for.
//!
//! This is also where the operating system's own text becomes an Entry Path,
//! and so where EP-1's normalization is applied: a name a filesystem hands back
//! decomposed is composed to NFC before anything is built out of it, because
//! two filesystems spell one file's name in two ways and the Library must not
//! hold it at two paths.
//!
//! How the operating system is asked for any of it is not here: the walk runs
//! over [`MappedRoots`](crate::MappedRoots), so a case can arrange a folder
//! whose listing, stat, or read fails at a chosen point — which is the only way
//! the verdicts above can be held to anything.
//!
//! It fails in [`LocalError`](crate::local_error::LocalError), which each flow
//! reports under its own names.

// The root check itself is asked by the walk, once per mapping, before anything
// under that root is read (spec: EP-12), and by a deletion before it moves this
// device's copies of the Entries it deleted out of that root (spec: EP-15).
mod assess_root;
pub(crate) use assess_root::assess_root;

mod root_refused;
pub(crate) use root_refused::RootRefused;

mod root_state;
pub(crate) use root_state::RootState;

// Which of the mapped folders one sync walks: every one, or the folders a
// drop wrote into (spec: EP-10, LA-34).
mod scan_scope;
pub use scan_scope::ScanScope;

mod source_file;
pub(crate) use source_file::SourceFile;

// What a writer that is gone left in a mapped folder (spec: OC-8, EP-11).
mod stale_scratch;
pub(crate) use stale_scratch::StaleScratch;

// What a scan left out because an operating system keeps it (spec: EP-17).
mod system_file;
pub(crate) use system_file::{kept_by_system, SystemFile};

// The files a drop placed with no birth time of their own; the walk records
// no birth time for a file it holds (spec: FM-9, EP-11).
mod unknown_births;
pub use unknown_births::UnknownBirths;
pub(crate) use unknown_births::NONE_PLACED;

mod walk_mappings;
pub(crate) use walk_mappings::{walk_mappings, walk_mappings_within};

mod walked;
pub(crate) use walked::{unavailable_roots, Walked};

mod walked_root;
pub(crate) use walked_root::WalkedRoot;
