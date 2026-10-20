use std::net::SocketAddr;

use super::Desktop;

impl Desktop {
    /// Where the explorer's host is bound, if a Library is open and its server
    /// has not stopped.
    pub fn explorer_address(&self) -> Option<SocketAddr> {
        self.shared.served().map(|opened| opened.explorer)
    }
}
