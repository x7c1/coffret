use std::net::SocketAddr;
use std::sync::Arc;

use coffret_explorer_host::credentials::Credentials;
use coffret_server::ServerState;

/// What is served once a Library is open: the server's state, and the
/// explorer's host in front of it.
pub(crate) struct Opened {
    /// What the server serves: what an unlock in place hands the reopened
    /// Library to, and what says whether one is needed.
    pub(crate) state: Arc<ServerState>,
    /// The handle that issues launch tickets, which stays with the core and is
    /// never handed to the host's routes (spec: LA-17).
    pub(crate) credentials: Credentials,
    /// Where the explorer's host is bound.
    pub(crate) explorer: SocketAddr,
}
