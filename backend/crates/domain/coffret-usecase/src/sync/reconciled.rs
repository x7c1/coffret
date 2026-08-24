use coffret_model::ContainerId;

/// A Container an earlier run spooled and never committed, and what this run
/// did about it.
///
/// The row it came from is the positive local provenance cleanup needs: it
/// names the batch that created the Container, and the absence of a Journal
/// record naming that Container is the other half of the proof (spec: OC-2,
/// OC-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reconciled {
    /// The Container the abandoned spool held.
    pub container_id: ContainerId,
    /// Whether its object was moved to the provider's trash.
    ///
    /// `false` where the earlier run never got as far as uploading — there was
    /// nothing on Storage to remove — where the Container turned out to be
    /// current after all, which is an earlier run whose commit landed and whose
    /// own refresh did not: the object is the Library's and is left alone — and
    /// where Storage refused the removal, which leaves an object no current
    /// state names for orphan cleanup to find (spec: OC-1, OC-4).
    pub trashed: bool,
}
