use coffret_model::ContainerId;

use crate::sync::source_file::SourceFile;

/// A local file this run will encode into a Container of its own.
///
/// The two shapes a sync produces differ in one field and nothing else, which
/// is why they are one type. An import has no Container to remove; a
/// replacement removes exactly the one-file Container that held the Entry
/// before, and its replacement is a new Container with a new ID rather than the
/// same Container rewritten (spec: CP-14, PK-12, PK-15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Candidate {
    /// The file to encode.
    pub(super) source: SourceFile,
    /// The one-file Container this one replaces, if any.
    pub(super) replaces: Option<ContainerId>,
}
