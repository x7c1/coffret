use coffret_format::EntryPlan;
use coffret_model::ContainerId;

use crate::local_scan::SourceFile;

/// A local file this run will pack, and the Container it displaces.
///
/// The two shapes a freeze selects differ in one field and nothing else, which
/// is why they are one type. A file not yet in the Library has nothing to
/// remove; a file whose current Entry is held by a one-file Container absorbs
/// that Container, which leaves the current set in the same batch that adds the
/// Pack (spec: PK-1, PK-7, CP-14).
#[derive(Debug, Clone)]
pub(super) struct Selected {
    /// The file to read.
    pub(super) source: SourceFile,
    /// What the Pack's entry table will say about it.
    ///
    /// Settled by the scan rather than by the spool, because a Pack's table is
    /// written before its content — which is the whole reason the content can
    /// stream (spec: PK-3, FM-9).
    pub(super) plan: EntryPlan,
    /// The one-file Container this Entry is absorbed out of, if any
    /// (spec: PK-1).
    pub(super) absorbs: Option<ContainerId>,
}
