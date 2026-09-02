use axum::http::header::HOST;
use axum::http::HeaderMap;

use super::{text, Admission};

impl Admission {
    /// Whether the `Host` names where this server is.
    ///
    /// Both spellings of one authority — with the port and without — and no
    /// name. `localhost` is not on the list and that is the point: a name is
    /// what somebody else's DNS can point at this socket, and no request that
    /// arrives by one is distinguishable here from a request that arrives by
    /// theirs.
    ///
    /// A request carrying no `Host` at all is refused with the rest. HTTP/1.1
    /// requires one, and a caller that omits it is not the explorer.
    pub(super) fn is_here(&self, headers: &HeaderMap) -> bool {
        let Some(host) = text(headers, HOST.as_str()) else {
            return false;
        };
        host == self.authority || host == self.host
    }
}
