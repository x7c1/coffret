use coffret_model::{ContainerId, EntryPath};

/// An Entry a fetch selected and did not place, and why.
///
/// None of these is an error and none of them is skipped quietly. A fetch may
/// only place a file at a path whose local state the device can vouch for, so
/// every path it declines has to say what it found instead — otherwise a run
/// that reported success would leave the user believing the folder is a copy of
/// the Library when parts of it are not (spec: EP-11, and the posture EP-4
/// sets).
///
/// The Entry Path travels in the value because the caller is what decides what
/// to do about it. It never travels into a log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Surfaced {
    /// A file stands at the target path and this device did not put it there.
    ///
    /// It is not this device's materialization of anything, so it may be an
    /// unsynced source file — content the Library has never held — and
    /// overwriting it would destroy it. Left byte-for-byte as it is: whether the
    /// Library's Entry or the local file wins is the user's to say, and a sync
    /// of that folder is what would offer the file to the Library instead
    /// (spec: EP-10, EP-11).
    ForeignFile {
        /// Where in the Library the occupied path stands.
        path: EntryPath,
    },
    /// This device materialized the path and the file no longer matches what it
    /// wrote down.
    ///
    /// Including one that is not there at all. Either way it is a pending local
    /// change the sync flow owns — a modification to offer the Library, or a
    /// deletion to report (spec: EP-10) — and re-fetching would quietly undo it.
    LocallyChanged {
        /// Where in the Library the changed file stands.
        path: EntryPath,
    },
    /// This device witnessed the file's deletion and is not putting it back.
    ///
    /// The row records that the device had the file and lost it (spec: EP-10).
    /// Restoring it is an explicit operation, the mirror of propagating the
    /// deletion on the sync side, and inferring either from the row alone is
    /// what neither flow does.
    WitnessedDeletion {
        /// Where in the Library the deleted file stood.
        path: EntryPath,
    },
    /// The committed Keyring records the key for this Entry's Container as lost.
    ///
    /// The Container stays current and its ciphertext stays where it is
    /// (spec: KL-17); what is gone is the envelope that would open it, so the
    /// Entry is reported locked rather than fetched (spec: KL-7, RV-2, RV-7).
    /// The rest of the run is unaffected: a locked Container costs its own
    /// Entries and nothing else.
    KeyLost {
        /// Where in the Library the locked Entry stands.
        path: EntryPath,
        /// The Container whose key the committed Keyring has none of.
        container_id: ContainerId,
    },
}

impl Surfaced {
    /// Where in the Library the finding is about.
    pub fn path(&self) -> &EntryPath {
        match self {
            Self::ForeignFile { path }
            | Self::LocallyChanged { path }
            | Self::WitnessedDeletion { path }
            | Self::KeyLost { path, .. } => path,
        }
    }
}
