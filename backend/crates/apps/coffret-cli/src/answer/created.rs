//! What `init` answers with.

use serde::Serialize;

use super::Storage;

/// What `init` created.
#[derive(Serialize)]
pub struct Created {
    library: String,
    library_id: String,
    storage: Storage,
    account: Option<String>,
    consent_asked: bool,
    recovery_code: String,
}

impl Created {
    /// The Library `library` as `init` created it.
    pub fn new(
        library: String,
        created: &coffret_device::CreatedLibrary,
        consent_asked: bool,
    ) -> Self {
        let provider = &created.settings.provider;
        Self {
            library,
            library_id: created.settings.library_id.to_string(),
            storage: Storage::from(provider),
            account: crate::storage_location::account(provider).map(str::to_owned),
            consent_asked,
            recovery_code: created.recovery_code.to_grouped_string(),
        }
    }
}
