use coffret_model::{ContainerKind, EntryLocation};

/// One Entry standing directly in a folder, with what a listing shows of it
/// beside its location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FolderFile {
    /// Where the Entry lives.
    pub location: EntryLocation,
    /// Whether this device has its file, by a present materialization record
    /// at its own path (spec: EP-10).
    pub present: bool,
    /// The kind of Container it lives in (spec: PK-15), or `None` where the
    /// catalog answered with the Entry and no summary of its Container — which
    /// two reads of a catalog another process commits between can do.
    pub container: Option<ContainerKind>,
}
