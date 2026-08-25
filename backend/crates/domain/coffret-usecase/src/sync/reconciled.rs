use coffret_model::ContainerId;

/// A Container an earlier run spooled and did not settle, and what this run
/// made of it.
///
/// The name predates the split of the two acts "reconcile" once covered: this
/// reports the *settle* act (spec: OC-7), not the *rebase* of a losing writer's
/// batch onto the new head (spec: CP-4).
///
/// The row it came from is the positive local provenance cleanup needs: it names
/// the batch that created the Container, and what the caught-up Index says about
/// that Container is the other half of the proof — and it cuts both ways. No
/// record naming it is proof the batch was abandoned, so the Container may be
/// disposed of (spec: OC-2, OC-3); the Container being current is proof the
/// record landed and that this device's own refresh is what did not, so the
/// bookkeeping is completed instead (spec: OC-7, CP-1).
///
/// Which of the two happened is reported and not silent, because they are
/// opposite outcomes for the caller: one says a Container left the Library's
/// Storage, the other says a file this device holds is accounted for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reconciled {
    /// The Container is current, so the interrupted commit's device-local
    /// bookkeeping was completed rather than reclaimed (spec: OC-7).
    ///
    /// The object is the Library's and is left where it is, the spool is gone
    /// because the Container it holds is committed, and the files this device
    /// materialized while producing the batch are marked present
    /// (spec: EP-10).
    Completed {
        /// The Container whose commit landed.
        container_id: ContainerId,
        /// How many of its current Entries this device now records as present.
        entries: usize,
    },
    /// No record names the Container, so the batch was abandoned and what it
    /// left behind was disposed of (spec: OC-2, OC-3).
    Disposed {
        /// The Container the abandoned spool held.
        container_id: ContainerId,
        /// Whether its object was moved to the provider's trash.
        ///
        /// `false` where the earlier run never got as far as uploading — there
        /// was nothing on Storage to remove — and where Storage refused the
        /// removal, which leaves an object no current state names for orphan
        /// cleanup to find (spec: OC-1, OC-4).
        trashed: bool,
    },
}

impl Reconciled {
    /// The Container this outcome is about.
    pub const fn container_id(&self) -> ContainerId {
        match self {
            Self::Completed { container_id, .. } | Self::Disposed { container_id, .. } => {
                *container_id
            }
        }
    }
}
