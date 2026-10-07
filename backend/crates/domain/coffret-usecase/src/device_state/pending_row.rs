use std::path::PathBuf;

use coffret_model::ContainerId;

use crate::device_state::batch_id::BatchId;
use crate::device_state::device_time::DeviceTime;
use crate::device_state::spool_state::SpoolState;

/// The device-local record of a Container this device is about to spool, has
/// spooled, or has uploaded before any commit.
///
/// Until the batch's Journal record exists, nothing it produced is part of the
/// current Container set (spec: CP-1) — and a Container sitting on Storage that
/// no reachable record mentions is not by itself evidence of an orphan, because
/// Storage may simply be withholding the record that made it current
/// (spec: OC-1). What makes cleanup safe is this row: local provenance naming
/// the batch that created the Container, so that a batch proven not to have
/// committed identifies exactly what may be removed (spec: OC-2, OC-3).
///
/// # When it is written
///
/// Before the spool file it names exists, and not after it is finished. A spool
/// step draws the Container ID, works out where the ciphertext will sit, and
/// records this row — and only then creates the file. So from the instant a
/// spool file can be on disk there is a row naming it, and an interruption
/// anywhere in the write, down to a kill the flow never sees, leaves state the
/// next sync can settle (spec: OC-2). What the row cannot say at that point
/// is whether the file is a whole Container, which is what
/// [`state`](Self::state) is for.
///
/// # When it goes
///
/// The row is deleted when the batch commits, which is
/// [`Index::refresh`](crate::Index::refresh)'s job, or when the batch is
/// abandoned. A third case is the one this row makes recoverable: a refresh that
/// failed after the record landed. The row then outlives a commit that did
/// happen, and a caught-up Index calling its Container current is proof of
/// exactly that — which is what lets the next run complete the bookkeeping the
/// refresh did not (spec: OC-7, CP-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingRow {
    /// A commit may have been sent. Absence from the Catalog cannot clear this
    /// evidence; uncertain Containers must be retained (spec: OC-1, OC-3).
    pub commit_attempted: bool,
    /// The Container the spool holds.
    pub container_id: ContainerId,
    /// Where the encrypted Container sits, or is about to sit, on this device.
    pub spool_path: PathBuf,
    /// The batch that created it (spec: OC-2).
    pub batch: BatchId,
    /// When this device announced the spool.
    pub created_at: DeviceTime,
    /// Whether the file at [`spool_path`](Self::spool_path) is a whole
    /// Container yet, and — once it is — where it was uploaded to, if it has
    /// been.
    pub state: SpoolState,
}
