//! What `join` answers with.

use serde::Serialize;

use coffret_device::FoundOnStorage;

use super::Storage;

/// What `join` took up.
///
/// No Recovery Code, for the reason the text form prints none: the one that
/// went in is the one that exists.
#[derive(Serialize)]
pub struct Joined {
    library: String,
    library_id: String,
    storage: Storage,
    account: Option<String>,
    consent_asked: bool,
    found_on_storage: &'static str,
}

impl Joined {
    /// The Library `library` as `join` took it up.
    pub fn new(
        library: String,
        joined: &coffret_device::JoinedLibrary,
        consent_asked: bool,
    ) -> Self {
        let provider = &joined.settings.provider;
        Self {
            library,
            library_id: joined.settings.library_id.to_string(),
            storage: Storage::from(provider),
            account: crate::storage_location::account(provider).map(str::to_owned),
            consent_asked,
            found_on_storage: match joined.found {
                FoundOnStorage::TheLibrary => "the_library",
                FoundOnStorage::NothingYet => "nothing_yet",
            },
        }
    }
}
