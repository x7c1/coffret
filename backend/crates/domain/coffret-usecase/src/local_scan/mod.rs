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
//! It fails in [`LocalError`](crate::local_error::LocalError), which each flow
//! reports under its own names.

// The root check itself is never named by a caller — the walk is what asks it,
// once per mapping, before anything under that root is read (spec: EP-12).
mod root_state;

mod source_file;
// `SourceReader` is what `SourceFile::open` answers with and is never named by a
// caller, so it is not re-exported here.
pub(crate) use source_file::SourceFile;

mod walk_mappings;
pub(crate) use walk_mappings::walk_mappings;

mod walked;
pub(crate) use walked::{unavailable_roots, RootState, Walked, WalkedRoot};
