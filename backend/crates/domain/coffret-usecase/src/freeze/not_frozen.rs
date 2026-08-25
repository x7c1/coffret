use coffret_model::{ContainerId, EntryPath};

/// A file the freeze found needing work it does not do.
///
/// Every one of these is update-eligible: its content differs from the Entry the
/// Library holds, or the Container holding that Entry cannot be opened at all
/// (spec: PK-11). Neither is an error and neither may be passed over quietly —
/// silence would tell the user that stale or unrecoverable content is safely
/// backed up, which is the one outcome the rule forbids outright (spec: PK-14).
///
/// What is *not* here is a file the freeze simply had no work for: an Entry a
/// Pack already holds and the local file still matches is neither eligible nor
/// a finding, and it is counted in
/// [`FreezeOutcome::packed_already`](super::FreezeOutcome::packed_already)
/// rather than named. So a folder frozen last month does not produce a finding
/// per file this month.
///
/// The Entry Path travels in the value because the caller is what decides what
/// to do about it. It never travels into a log line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotFrozen {
    /// The file changed, and its current Entry lives in a Pack.
    ///
    /// An Entry already in a Pack is never eligible for a freeze: existing Packs
    /// are neither read nor rewritten by one (spec: PK-1, PK-2). Carrying the
    /// change into the Library means read-modify-replace over that Pack, which
    /// is `update`'s (spec: PK-10, PK-11, PK-12). So the Pack is left
    /// byte-for-byte as it is and the file is reported.
    ModifiedInPack {
        /// Where in the Library the changed file stands.
        path: EntryPath,
        /// The Pack holding its current Entry.
        container_id: ContainerId,
    },
    /// The Entry lives in a Pack the committed Keyring records no key for.
    ///
    /// The stored ciphertext is unreadable, so re-encrypting the local plaintext
    /// is the only content-recovery path — and over a Pack that is
    /// read-modify-replace, which a freeze does not do (spec: KL-7, PK-10,
    /// PK-11). Reported whether or not the local file differs: under a lost key
    /// there is nothing to compare it against.
    KeyLostInPack {
        /// Where in the Library the unreadable Entry stands.
        path: EntryPath,
        /// The Pack whose key the Library records as lost.
        container_id: ContainerId,
    },
}
