use coffret_model::{ContainerSummary, DepositedAt, EntryMetadata, KeyEnvelope};

/// One Container to replace, and which of its Entries the replacement keeps.
pub(crate) struct Replacing<'a> {
    /// The old Container, as the catalog records it.
    pub(crate) old: &'a ContainerSummary,
    /// The envelope the committed Keyring maps it to (spec: KL-7).
    pub(crate) envelope: &'a KeyEnvelope,
    /// Its rows as the catalog records them, in the order they occupy its
    /// stream (spec: FM-9). Every one of them, so that a row's position here
    /// is its entry number, which is how the replacement names the Entries it
    /// carries forward.
    pub(crate) table: &'a [EntryMetadata],
    /// When each of those rows' content was deposited, row for row. A kept
    /// row carries its time into the replacement: a rewrite puts no content
    /// into the Library (spec: PK-10, FM-15).
    pub(crate) deposit_times: &'a [DepositedAt],
    /// Which of those rows the replacement carries, one flag per row. At least
    /// one is set: a Container with nothing left to carry is removed, never
    /// replaced by an empty one (spec: PK-10, FM-10).
    pub(crate) keep: &'a [bool],
}
