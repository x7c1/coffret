use coffret_model::ControlObjectName;

use crate::commit::commit_error::CommitError;

/// What the checkpoint policy did after a commit (spec: CK-8, CK-10, CK-11).
///
/// A checkpoint is not part of the commit: the record is the commit point
/// (spec: CP-1), and a Snapshot that is not written leaves the records it would
/// have covered replayable. So none of these is a failure of the commit, and the
/// one that carries an error carries it as the account of what did not happen
/// rather than as something the caller has to undo.
#[derive(Debug)]
pub enum CheckpointOutcome {
    /// The Journal past the newest checkpoint has not grown past the threshold,
    /// so no Snapshot was written (spec: CK-8).
    NotDue,
    /// This commit wrote the Snapshot of the head it became (spec: CK-10).
    Written {
        /// The name it was created under.
        object: ControlObjectName,
    },
    /// Another writer had already put a Snapshot of this head in the slot.
    ///
    /// Losing that conditional create is not a failure: two Snapshots of one
    /// head are the same checkpoint, so the one already there settles it and
    /// this device's upload is done (spec: CK-11).
    Existing {
        /// The name the sibling was created under.
        object: ControlObjectName,
    },
    /// The Snapshot could not be written, and the commit stands regardless.
    ///
    /// The next qualifying moment writes one (spec: CK-8), so this is reported
    /// rather than retried here.
    Failed {
        /// What stopped it.
        cause: Box<CommitError>,
    },
}
