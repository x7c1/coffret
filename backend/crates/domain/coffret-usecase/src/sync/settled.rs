use coffret_model::ContainerId;

use crate::sync::disposal::Disposal;

/// A Container an earlier run spooled and did not settle, and what this run
/// made of it: the report of the *settle* act (spec: OC-7), not of the *rebase*
/// of a losing writer's batch onto the new head (spec: CP-4).
///
/// The row it came from is the positive local provenance cleanup needs: it names
/// the batch that created the Container, and it says whether the spool it names
/// is a whole Container — whether the row calls it `Spooled`. Where it does not,
/// the row is the whole of the proof: only a spool a row calls `Spooled` is ever
/// uploaded, so nothing can have committed that Container and it is reclaimed
/// with no question put to the Library at all (spec: OC-2).
///
/// Where it does, what a caught-up Index says about that Container is the other
/// half of the proof — and it cuts both ways. No record naming it is proof the
/// batch was abandoned, so the Container may be disposed of (spec: OC-2, OC-3);
/// the Container being current is proof the record landed and that this device's
/// own refresh is what did not, so the bookkeeping is completed instead
/// (spec: OC-7, CP-1).
///
/// Which of the two happened is reported and not silent, because they are
/// opposite outcomes for the caller: one says an abandoned batch was reclaimed
/// — its object on Storage too, unless Storage refused the trash
/// ([`Disposal`]) — the other says a file this device holds is accounted for.
///
/// There is deliberately no `PartialEq`: a disposal Storage refused carries what
/// Storage answered, and error values are reported rather than compared.
#[derive(Debug, Clone)]
pub enum Settled {
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
    /// Nothing committed the Container, so what its batch left on this device
    /// was disposed of, and its object on Storage with it unless Storage refused
    /// the trash (spec: OC-2, OC-3).
    ///
    /// Two rows reach this, and what proves it differs. A row still
    /// [`Spooling`](crate::device_state::SpoolState::Spooling) proves it on its
    /// own: such a Container was never uploaded, so no record can name it, and
    /// neither the current set nor the Library's head is consulted. A row that
    /// calls its spool `Spooled` is disposed of because no record in a caught-up
    /// Index names its Container, which is proof the batch was abandoned
    /// (spec: OC-3).
    Disposed {
        /// The Container the abandoned spool was for.
        container_id: ContainerId,
        /// What became of the object it left on Storage, if it left one.
        disposal: Disposal,
    },
}

impl Settled {
    /// The Container this outcome is about.
    pub const fn container_id(&self) -> ContainerId {
        match self {
            Self::Completed { container_id, .. } | Self::Disposed { container_id, .. } => {
                *container_id
            }
        }
    }
}
