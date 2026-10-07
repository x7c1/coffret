use coffret_model::ContainerId;

use crate::sync::disposal::Disposal;

/// What settlement proved about an interrupted run (spec: OC-2, OC-3, OC-7).
/// Unknown commit outcomes retain their spool, object, and provenance.
#[derive(Debug, Clone)]
pub enum Settled {
    /// A commit may have reached Storage; absence from the Catalog proves
    /// nothing about it. All provenance and ciphertext are retained.
    Retained {
        /// The Container whose commit outcome is uncertain.
        container_id: ContainerId,
    },
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
            Self::Retained { container_id }
            | Self::Completed { container_id, .. }
            | Self::Disposed { container_id, .. } => *container_id,
        }
    }
}
