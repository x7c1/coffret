//! What a device records about where one Library lives.
//!
//! The file is the contract between the things that open a Library — this
//! crate's own calls today, the browser-based explorer later — rather than one
//! command's private note, so its shape is versioned and a version this build
//! does not know is refused rather than guessed at.
//!
//! What is *not* in it is as deliberate as what is. No paths, because the
//! directory layout already says where each piece of a Library is and a second
//! answer could disagree with the first. No S3 credentials, because the AWS
//! SDK's own resolution — the environment, then a profile — is where those
//! belong and copying them here would put a long-lived secret in a file that
//! only needs to say *where* the bucket is. The Drive client secret is the one
//! credential-looking thing that is here, and it is here because Google's
//! desktop OAuth model treats it as non-confidential — it ships inside every
//! copy of such a client — while the refresh the explorer will do needs it.

use serde::{Deserialize, Serialize};

use coffret_model::LibraryId;

mod library_id_hex;

mod provider_settings;
pub use provider_settings::ProviderSettings;

mod read;
mod write;

#[cfg(test)]
mod tests;

/// One Library's entry in a device's own configuration.
///
/// Which Library this is, where on Storage it is, and a copy of what it is
/// called. Everything else a device needs — the Master Key, the catalog, the
/// spool — is found by the directory layout, and everything the Library itself
/// knows is on Storage under the Master Key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSettings {
    /// The shape this file is in. [`write`](Self::write) records
    /// [`VERSION`](Self::VERSION), and [`read`](Self::read) refuses anything
    /// else.
    pub version: u32,
    /// The Library this device is configured for.
    ///
    /// It is configuration and not key material: it is drawn from a CSPRNG at
    /// creation, it takes no input from the Master Key, and it survives a
    /// rotation unchanged — a name Storage can read must not be a function of
    /// a key, and rotating one must not rename the folder every other device
    /// is configured against (spec: FM-18).
    #[serde(with = "library_id_hex")]
    pub library_id: LibraryId,
    /// Where on Storage the Library's objects are.
    pub provider: ProviderSettings,
    /// A copy of the Library's name, as the catalog held it when the Library
    /// was last created, joined or unlocked here (spec: FM-15).
    ///
    /// The name itself is the Library's and is kept on Storage under the
    /// Master Key, which nothing can reach before an unlock; this copy is what
    /// lets a device list its Libraries by name, and find one by name, before
    /// that. It is written again at every unlock, so a name another device
    /// gave the Library reaches this file the next time it is opened here.
    /// Absent on a Library whose catalog names none yet, which is what a build
    /// before names wrote; a reader that does not know the field ignores it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The name of the directory an earlier build kept this Library in, where
    /// a server moved the directory to the Library's ID before an unlock gave
    /// the Library that name (spec: FM-15).
    ///
    /// Kept apart from [`name`](Self::name), which is only ever copied from
    /// the catalog. Until the unlock that finds the catalog naming the Library,
    /// which drops it, it is what the Library is listed and found by.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub earlier_name: Option<String>,
}

impl DeviceSettings {
    /// The version this build writes and reads.
    pub const VERSION: u32 = 1;

    /// Settings for a Library at this build's version, with no copy of its
    /// name yet.
    pub fn new(library_id: LibraryId, provider: ProviderSettings) -> Self {
        Self {
            version: Self::VERSION,
            library_id,
            provider,
            name: None,
            earlier_name: None,
        }
    }

    /// The same settings, keeping `name` as the copy of the Library's name.
    pub fn named(mut self, name: Option<String>) -> Self {
        self.name = name;
        self
    }
}
