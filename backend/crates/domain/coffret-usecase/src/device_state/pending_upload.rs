use std::path::PathBuf;

use coffret_model::{ContainerId, ObjectRef};

use crate::device_state::batch_id::BatchId;
use crate::device_state::device_time::DeviceTime;

/// A Container this device encrypted, and perhaps uploaded, before any commit.
///
/// Until the batch's Journal record exists, nothing it produced is part of the
/// current Container set (spec: CP-1) — and a Container sitting on Storage that
/// no reachable record mentions is not by itself evidence of an orphan, because
/// Storage may simply be withholding the record that made it current
/// (spec: OC-1). What makes cleanup safe is this row: local provenance naming
/// the batch that created the Container, so that a batch proven not to have
/// committed identifies exactly what may be removed (spec: OC-2, OC-3).
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
    /// Where the encrypted Container sits on this device.
    pub spool_path: PathBuf,
    /// The batch that created it (spec: OC-2).
    pub batch: BatchId,
    /// When this device created the spool.
    pub created_at: DeviceTime,
    /// Where the Container was uploaded to, once it has been.
    ///
    /// `None` means the ciphertext exists only in the spool, so abandoning the
    /// batch removes a local file and nothing on Storage.
    pub object_ref: Option<ObjectRef>,
}
