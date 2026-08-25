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
//! It fails in [`LocalError`](crate::local_error::LocalError), which each flow
//! reports under its own names.

mod source_file;
// `SourceReader` is what `SourceFile::open` answers with and is never named by a
// caller, so it is not re-exported here.
pub(crate) use source_file::SourceFile;

mod walk_mappings;
pub(crate) use walk_mappings::walk_mappings;
