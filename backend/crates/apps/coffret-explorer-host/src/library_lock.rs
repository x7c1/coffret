use coffret_server::ServerState;

/// Whether the Library behind the host is locked (spec: DK-1), as the host
/// asks it before forwarding (spec: LA-14).
///
/// Read from the server's own state rather than kept by the host beside it,
/// so that there is one answer to the question and not two that could
/// disagree. The host's `423` is for the page's sake: what enforces the lock is
/// the server's own custody of the keys, which refuses keyed work whatever the
/// host let through.
pub trait LibraryLock: Send + Sync {
    /// Whether the Library is locked right now.
    fn is_locked(&self) -> bool;
}

impl LibraryLock for ServerState {
    fn is_locked(&self) -> bool {
        // Asked without a key handle, so a page asking records no presence and
        // defers no lock (spec: DK-4).
        !self.holds_library()
    }
}
