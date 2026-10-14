use std::io;

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
    ///
    /// What is said when there is none is why, in fixed words: never where the
    /// file is, whose path names the Library's directory on this device, and
    /// never the read's own message, which may name it too (spec: EL-1,
    /// spec: EL-3).
    fn current_key(&self) -> Option<HeaderValue> {
        let key = match ServerKey::read_published(&self.key_file) {
            Ok(key) => key,
            Err(cause) => {
                tracing::warn!(
                    problem = KeyProblem::of(&cause).as_str(),
                    io_error_kind = ?cause.kind(),
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
                    problem = KeyProblem::Malformed.as_str(),
                    "the server's key could not be read; /api is forwarded without one",
                );
                None
            }
        }
    }
}

/// Why there is no key to forward with.
#[derive(Debug, Clone, Copy)]
enum KeyProblem {
    /// No file where the server publishes its key: no server has served this
    /// Library on this device yet. A server that stops leaves its file behind,
    /// spent rather than removed (spec: LA-4).
    Missing,
    /// A file is there and could not be read.
    Unreadable,
    /// The file was read and what it holds cannot be a key.
    Malformed,
}

impl KeyProblem {
    fn of(cause: &io::Error) -> Self {
        match cause.kind() {
            io::ErrorKind::NotFound => Self::Missing,
            // Not UTF-8, which a key the server drew always is.
            io::ErrorKind::InvalidData => Self::Malformed,
            _ => Self::Unreadable,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Unreadable => "unreadable",
            Self::Malformed => "malformed",
        }
    }
}
