use axum::http::header::{HOST, ORIGIN};
use axum::http::{HeaderMap, HeaderValue};
use coffret_device::ServerKey;
use coffret_server::SERVER_KEY_HEADER;

use super::{remove_hop_by_hop, Forwarder};

impl Forwarder {
    /// The headers the server is sent for a request that arrived with
    /// `headers`. See [`crate::router`] for each step.
    pub(super) fn headers(&self, mut headers: HeaderMap) -> HeaderMap {
        let own_origin = headers
            .get(HOST)
            .and_then(|host| host.to_str().ok())
            .map(|host| format!("http://{host}"));
        let from_own_page = match (headers.get(ORIGIN), own_origin) {
            (Some(origin), Some(own)) => origin.as_bytes() == own.as_bytes(),
            _ => false,
        };

        remove_hop_by_hop(&mut headers);

        // Removed before it is set, so that nothing a caller of this host put
        // under this name is what gets forwarded.
        headers.remove(SERVER_KEY_HEADER);
        if let Some(key) = self.current_key() {
            headers.insert(SERVER_KEY_HEADER, key);
        }

        headers.insert(HOST, self.server_host.clone());
        if from_own_page {
            headers.insert(ORIGIN, self.server_origin.clone());
        }
        headers
    }

    /// The key in the file right now, or `None` where there is none to send.
    fn current_key(&self) -> Option<HeaderValue> {
        let key = match ServerKey::read_published(&self.key_file) {
            Ok(key) => key,
            Err(cause) => {
                tracing::warn!(
                    key_file = %self.key_file.display(),
                    %cause,
                    "the server's key could not be read; /api is forwarded without one",
                );
                return None;
            }
        };
        match HeaderValue::from_str(&key) {
            Ok(value) => Some(value),
            Err(_) => {
                // Never the contents: whatever is in that file is the secret.
                tracing::warn!(
                    key_file = %self.key_file.display(),
                    "the server's key file holds something that cannot be a header; /api is forwarded without a key",
                );
                None
            }
        }
    }
}
