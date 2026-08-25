use std::path::PathBuf;

use coffret_model::{ContainerId, ObjectRef};

use crate::device_state::batch_id::BatchId;
use crate::device_state::device_time::DeviceTime;
use crate::device_state::pending_spool_state::PendingSpoolState;

/// A Container this device is about to write, has written, or has uploaded,
/// before any commit.
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
/// next reconcile can settle (spec: OC-2). What the row cannot say at that point
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
pub struct PendingUpload {
    /// The Container the spool holds.
    pub container_id: ContainerId,
    /// Where the encrypted Container sits, or is about to sit, on this device.
    pub spool_path: PathBuf,
    /// The batch that created it (spec: OC-2).
    pub batch: BatchId,
    /// When this device announced the spool.
    pub created_at: DeviceTime,
    /// Whether the file at [`spool_path`](Self::spool_path) is a whole
    /// Container yet.
    ///
    /// It ties one invariant to [`object_ref`](Self::object_ref): a Container is
    /// uploaded only after its spool is complete, so
    /// [`Writing`](PendingSpoolState::Writing) always comes with an
    /// `object_ref` of `None`, and an `object_ref` is only ever set on a
    /// [`Written`](PendingSpoolState::Written) row.
    pub state: PendingSpoolState,
    /// Where the Container was uploaded to, once it has been.
    ///
    /// `None` means the ciphertext exists only in the spool, so abandoning the
    /// batch removes a local file and nothing on Storage.
    pub object_ref: Option<ObjectRef>,
}
