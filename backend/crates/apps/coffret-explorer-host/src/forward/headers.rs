use axum::http::header::{AUTHORIZATION, COOKIE, HOST, ORIGIN};
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use coffret_server::SERVER_KEY_HEADER;

use super::{remove_hop_by_hop, Forwarder};

/// What a caller says a proxy in front of it said, which nothing here trusts
/// (spec: LA-13).
const FORWARDED: &str = "forwarded";
const X_FORWARDED: &str = "x-forwarded-";

impl Forwarder {
    /// The headers the server is sent for an admitted request that arrived
    /// with `headers`, forwarded with `key` (spec: LA-14).
    ///
    /// `from_own_page` is whether its `Origin` was verified as the host's own,
    /// which is the only case one is sent on, as the server's.
    pub(super) fn headers(
        &self,
        mut headers: HeaderMap,
        key: HeaderValue,
        from_own_page: bool,
    ) -> HeaderMap {
        remove_hop_by_hop(&mut headers);

        // The browser's credential never reaches the server, and nothing a
        // caller put under the key's name is what gets forwarded (spec: LA-15).
        headers.remove(AUTHORIZATION);
        headers.remove(COOKIE);
        headers.remove(SERVER_KEY_HEADER);
        headers.remove(FORWARDED);
        let forwarded: Vec<HeaderName> = headers
            .keys()
            .filter(|name| name.as_str().starts_with(X_FORWARDED))
            .cloned()
            .collect();
        for name in forwarded {
            headers.remove(name);
        }

        headers.insert(SERVER_KEY_HEADER, key);
        headers.insert(HOST, self.server_host.clone());
        if from_own_page {
            headers.insert(ORIGIN, self.server_origin.clone());
        } else {
            headers.remove(ORIGIN);
        }
        headers
    }
}
