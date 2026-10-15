use std::io;
use std::sync::{Arc, Mutex, PoisonError};

use axum::http::HeaderValue;

use crate::credentials::InstanceId;
use crate::KeySource;

/// The server key one generation of sessions was issued under (spec: LA-15).
///
/// Read as the generation's first session is made, and held: every request a
/// session of it sends is forwarded with that key and no other. The file is
/// read again for each one, and a key that has changed since — a server of
/// another run behind the port (spec: LA-4) — is not used on behalf of a token
/// issued while the older one stood. The host fails closed instead.
pub(crate) struct KeyPin {
    source: Arc<dyn KeySource>,
    pinned: Mutex<Option<Pinned>>,
}

struct Pinned {
    generation: InstanceId,
    key: HeaderValue,
}

/// Why the key could not be confirmed.
#[derive(Debug, Clone, Copy)]
pub(crate) enum KeyProblem {
    /// No file where the server publishes its key.
    Missing,
    /// A file is there and could not be read.
    Unreadable(io::ErrorKind),
    /// The file was read and what it holds cannot be a key.
    Malformed,
    /// The key is not the one this generation's sessions were issued under.
    Changed,
    /// No key was pinned for this generation: it has been retired since.
    Unpinned,
}

impl KeyPin {
    pub(crate) fn new(source: Arc<dyn KeySource>) -> Self {
        Self {
            source,
            pinned: Mutex::new(None),
        }
    }

    /// Pins the key as it stands for `generation`, where none is pinned for it
    /// yet, and confirms it where one is.
    pub(crate) fn pin(&self, generation: &InstanceId) -> Result<(), KeyProblem> {
        let mut pinned = self.pinned.lock().unwrap_or_else(PoisonError::into_inner);
        let key = self.read();
        match (pinned.as_ref(), key) {
            (Some(held), Ok(key)) if held.generation == *generation => {
                if same(&held.key, &key) {
                    Ok(())
                } else {
                    *pinned = None;
                    Err(KeyProblem::Changed)
                }
            }
            (_, Ok(key)) => {
                *pinned = Some(Pinned {
                    generation: generation.clone(),
                    key,
                });
                Ok(())
            }
            (_, Err(problem)) => {
                *pinned = None;
                Err(problem)
            }
        }
    }

    /// The key to forward a request of a session of `generation` with: the one
    /// pinned for it, where the file still holds that one.
    pub(crate) fn key_for(&self, generation: &InstanceId) -> Result<HeaderValue, KeyProblem> {
        let mut pinned = self.pinned.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(held) = pinned
            .as_ref()
            .filter(|held| held.generation == *generation)
        else {
            return Err(KeyProblem::Unpinned);
        };
        match self.read() {
            Ok(key) if same(&held.key, &key) => Ok(held.key.clone()),
            Ok(_) => {
                *pinned = None;
                Err(KeyProblem::Changed)
            }
            Err(problem) => {
                *pinned = None;
                Err(problem)
            }
        }
    }

    /// The key in the file right now, as the header it is sent in.
    fn read(&self) -> Result<HeaderValue, KeyProblem> {
        let key = self.source.read().map_err(|cause| match cause.kind() {
            io::ErrorKind::NotFound => KeyProblem::Missing,
            // Not UTF-8, which a key the server drew always is.
            io::ErrorKind::InvalidData => KeyProblem::Malformed,
            kind => KeyProblem::Unreadable(kind),
        })?;
        if key.is_empty() {
            return Err(KeyProblem::Malformed);
        }
        let mut value = HeaderValue::from_str(&key).map_err(|_| KeyProblem::Malformed)?;
        value.set_sensitive(true);
        Ok(value)
    }
}

impl KeyProblem {
    /// The fixed word an event records it by (spec: EL-1).
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Unreadable(_) => "unreadable",
            Self::Malformed => "malformed",
            Self::Changed => "changed",
            Self::Unpinned => "unpinned",
        }
    }

    /// The I/O error's kind, where the file could not be read, and nothing
    /// about it beyond that (spec: EL-3).
    pub(crate) fn io_error_kind(self) -> Option<io::ErrorKind> {
        match self {
            Self::Missing => Some(io::ErrorKind::NotFound),
            Self::Unreadable(kind) => Some(kind),
            Self::Malformed | Self::Changed | Self::Unpinned => None,
        }
    }
}

/// Whether two keys are the same, compared without stopping at the first byte
/// that differs (spec: LA-7).
fn same(one: &HeaderValue, other: &HeaderValue) -> bool {
    let (one, other) = (one.as_bytes(), other.as_bytes());
    one.len() == other.len()
        && one
            .iter()
            .zip(other)
            .fold(0_u8, |sofar, (a, b)| sofar | (a ^ b))
            == 0
}
