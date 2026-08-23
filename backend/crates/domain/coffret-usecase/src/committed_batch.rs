use crate::device_state::LocalObservation;
use crate::journal_record::JournalRecord;

/// A batch of this device's own that has just committed.
///
/// It is the same content a replaying device would see — the record — plus the
/// one thing only the committing device knows: which local files it put in
/// place while producing the batch. That is what
/// [`Index::refresh`](crate::Index::refresh) needs beyond
/// [`apply`](crate::Index::apply), and it is device state rather than Library
/// content, so no Snapshot ever carries it (spec: CK-7, EP-10).
///
/// A batch reaches this shape only after its Journal record was successfully
/// created: before that the batch has changed nothing (spec: CP-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedBatch {
    /// The record that committed the batch.
    pub record: JournalRecord,
    /// The local files this device materialized as part of it.
    ///
    /// A commit does not imply that every Entry it adds is on this device's
    /// disk: a repack commits Containers whose Entries the device may never
    /// have held, and only the files it actually put in place may later be
    /// reported as deleted locally (spec: EP-10). So the batch says which ones
    /// those are rather than leaving it to be inferred from the additions.
    pub materialized: Vec<LocalObservation>,
}
