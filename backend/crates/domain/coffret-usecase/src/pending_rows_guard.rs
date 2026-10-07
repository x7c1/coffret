/// Exclusive ownership of this device's pending rows (spec: OC-2).
/// Keep it until all spooling, commit attempts, and settlement finish. Dropping
/// the guard releases ownership, including when the operation is cancelled.
#[must_use = "keep the guard for the whole import or settlement"]
pub struct PendingRowsGuard {
    _owner: Box<dyn Send + Sync>,
}

impl PendingRowsGuard {
    /// Holds a backend's RAII lock without exposing its implementation.
    pub fn holding(owner: impl Send + Sync + 'static) -> Self {
        Self {
            _owner: Box::new(owner),
        }
    }
}
