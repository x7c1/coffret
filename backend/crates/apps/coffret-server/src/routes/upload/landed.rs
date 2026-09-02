use coffret_device::EntryPath;

/// One part that landed.
pub(super) struct Landed {
    pub(super) path: EntryPath,
    pub(super) bytes: u64,
}
