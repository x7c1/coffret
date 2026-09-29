//! Where a Library is on Storage, as the answer carries it.

use serde::Serialize;

use coffret_device::ProviderSettings;

/// Where a Library is on Storage, in the provider's own terms.
///
/// What `join` is given to find it, which is why it is here at all: the S3
/// prefix is the Library's own, ending in its `coffret-<library id>/` folder,
/// and the Drive folder is the app folder's id (spec: FM-18). Nothing about how
/// to sign for it is here — neither an endpoint a person may have chosen for a
/// reason of their own, nor anything of the client a Drive Library authorizes
/// as.
#[derive(Serialize)]
#[serde(tag = "provider", rename_all = "snake_case")]
pub enum Storage {
    /// An S3 bucket.
    S3 {
        /// The bucket.
        bucket: String,
        /// The Library's own prefix in it.
        prefix: String,
    },
    /// A Google Drive folder.
    Drive {
        /// The app folder's id.
        folder_id: String,
    },
}

impl From<&ProviderSettings> for Storage {
    fn from(provider: &ProviderSettings) -> Self {
        match provider {
            ProviderSettings::S3 { bucket, prefix, .. } => Self::S3 {
                bucket: bucket.clone(),
                prefix: prefix.clone(),
            },
            ProviderSettings::Drive { folder_id, .. } => Self::Drive {
                folder_id: folder_id.clone(),
            },
        }
    }
}
