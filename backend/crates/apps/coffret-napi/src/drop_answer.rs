use coffret_desktop_core::Dropped;
use napi_derive::napi;

use crate::{DroppedFileRow, NotTakenRow};

/// What a drop the shell handed over is answered with, for the explorer's
/// page: the one-time handle its import names, and what each file is. No path
/// on this device is in it.
///
/// Nullable rather than optional: a drop that took nothing reaches the shell
/// with `handle: null`, as the shell's `addon.ts` declares it, and not with no
/// `handle` at all, which is what napi-rs makes of a `None` field otherwise.
#[napi(object, use_nullable = true)]
pub struct DropAnswer {
    /// The handle, or `null` where nothing was taken and nothing is held.
    pub handle: Option<String>,
    /// Every file taken; the import names each by its index here.
    pub files: Vec<DroppedFileRow>,
    /// Everything met and not taken.
    pub not_taken: Vec<NotTakenRow>,
}

impl From<Dropped> for DropAnswer {
    fn from(dropped: Dropped) -> Self {
        Self {
            handle: dropped.handle,
            files: dropped
                .files
                .into_iter()
                .map(|row| DroppedFileRow {
                    path: row.path,
                    // Exact below 2^53 bytes, which no file reaches.
                    size: row.size as f64,
                    mtime: row.mtime_millis,
                })
                .collect(),
            not_taken: dropped
                .not_taken
                .into_iter()
                .map(|row| NotTakenRow {
                    path: row.path,
                    reason: row.reason.to_owned(),
                })
                .collect(),
        }
    }
}
