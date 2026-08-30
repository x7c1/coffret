//! Where a Library's objects are, in the words the provider uses for it.
//!
//! Shared by `init` and `join`, which is why it is here rather than in either
//! of them: the two commands say where a Library is in one voice or a person
//! cannot match one answer to the other.

use coffret_device::ProviderSettings;

/// Where the Library's objects are, in the words the provider uses for it.
///
/// Said back because `init` promises a Library "on this device and on Storage"
/// and nothing else in the report covers the second half — and because the
/// prefix it prints on S3 is exactly what a second device is given to `join`
/// with (spec: FM-18).
pub fn storage(provider: &ProviderSettings) -> String {
    match provider {
        ProviderSettings::Drive { folder_id, .. } => {
            format!("the Google Drive folder {folder_id}")
        }
        ProviderSettings::S3 { bucket, prefix, .. } => format!("s3://{bucket}/{prefix}"),
    }
}
