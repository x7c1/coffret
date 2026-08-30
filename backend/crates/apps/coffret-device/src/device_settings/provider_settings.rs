use serde::{Deserialize, Serialize};

/// Where on Storage one Library's objects are.
///
/// Tagged by `kind`, and a `kind` this build has no reading for is refused
/// rather than skipped over: a device that could not tell which provider a
/// Library is on has no way to reach it, and pretending otherwise would turn a
/// newer settings file into a Library that appears to be empty.
///
/// Neither variant carries a credential that a device could not obtain again
/// for itself. The Drive client secret is the exception that proves it: an
/// installed application's secret ships inside every copy of that application,
/// which is why the flow is PKCE-protected rather than resting on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ProviderSettings {
    /// A Library in a Google Drive folder.
    #[serde(rename = "drive")]
    Drive {
        /// The app folder this Library's objects live in (spec: FM-18).
        ///
        /// Drive names files by an id it mints, so the folder is recorded by
        /// id: the name says which Library it is, and only the id says which
        /// folder.
        folder_id: String,
        /// The OAuth client the grant was given to.
        client_id: String,
        /// The client secret, for a client registered with one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_secret: Option<String>,
    },
    /// A Library under a prefix of an S3 bucket.
    #[serde(rename = "s3")]
    S3 {
        /// The bucket the Library is in.
        bucket: String,
        /// The prefix every one of the Library's keys starts with.
        ///
        /// The base the user chose with `coffret-<library id>/` after it, which
        /// is the app folder's name as a key prefix (spec: FM-18). It is
        /// recorded rather than recomputed because the base is the user's and
        /// nothing else on the device remembers it.
        prefix: String,
        /// The S3 endpoint to talk to, where it is not AWS's own.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        endpoint: Option<String>,
        /// The region to sign for.
        ///
        /// Absent leaves the SDK's own resolution — the environment, then a
        /// profile — to decide, the same way the credentials are left to it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        region: Option<String>,
        /// Whether the bucket is addressed as a path segment rather than as a
        /// subdomain, which is what an S3 implementation on a host of its own
        /// needs.
        path_style: bool,
    },
}

impl ProviderSettings {
    /// What this provider is called in the settings file.
    ///
    /// For a message that has to say which provider a Library is on without
    /// showing what the settings hold about it.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Drive { .. } => "drive",
            Self::S3 { .. } => "s3",
        }
    }
}
