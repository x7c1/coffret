use coffret_explorer_host::LibraryLock;

/// The lock of a server in another process, which this one cannot see.
///
/// The host forwards as though the Library were unlocked, and the server's own
/// custody of the keys refuses what a locked Library cannot answer, with the
/// locked refusal the page already knows. The host's own `423` is for the
/// page's sake (spec: LA-14); what enforces the lock is the server either way.
pub struct InAnotherProcess;

impl LibraryLock for InAnotherProcess {
    fn is_locked(&self) -> bool {
        false
    }
}
