//! What `authorize` answers with.

use serde::Serialize;

/// What `authorize` renewed.
#[derive(Serialize)]
pub struct Authorized {
    consent_asked: bool,
}

impl Authorized {
    /// A renewal that did or did not have to ask for a consent.
    pub fn new(consent_asked: bool) -> Self {
        Self { consent_asked }
    }
}
