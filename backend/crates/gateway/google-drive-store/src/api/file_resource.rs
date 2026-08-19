use coffret_model::Mtime;
use coffret_usecase::{ObjectInfo, ObjectRef, ProviderHash};
use serde::Deserialize;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// The fields the gateway asks Drive for about a file.
///
/// Drive returns a small default set unless told otherwise, so every call names
/// exactly what the port reports and nothing more.
pub const FILE_FIELDS: &str = "id,name,size,md5Checksum,modifiedTime";

/// The fields a listing asks for, which is the file set plus the page marker.
pub const LIST_FIELDS: &str = "nextPageToken,files(id,name,size,md5Checksum,modifiedTime)";

/// One file as Drive describes it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResource {
    /// Drive's identifier for the file, which is what an [`ObjectRef`] carries.
    pub id: String,
    /// The name the file was created under.
    pub name: Option<String>,
    /// The stored size. Drive spells 64-bit integers as strings in JSON.
    pub size: Option<String>,
    /// Drive's MD5 of the stored bytes.
    pub md5_checksum: Option<String>,
    /// When the file last changed, as RFC 3339.
    pub modified_time: Option<String>,
}

impl FileResource {
    /// What the port reports about this file.
    pub fn to_object_info(&self) -> ObjectInfo {
        ObjectInfo {
            object_ref: ObjectRef::new(&self.id),
            name: self.name.clone().unwrap_or_default(),
            size: self
                .size
                .as_deref()
                .and_then(|size| size.parse().ok())
                .unwrap_or_default(),
            mtime: self
                .modified_time
                .as_deref()
                .and_then(to_mtime)
                .unwrap_or_else(|| Mtime::from_unix_seconds(0)),
            hash: self.md5_checksum.as_deref().map(ProviderHash::new),
        }
    }
}

/// A modification time, from RFC 3339 to whole seconds from the Unix epoch.
fn to_mtime(timestamp: &str) -> Option<Mtime> {
    OffsetDateTime::parse(timestamp, &Rfc3339)
        .ok()
        .map(|at| Mtime::from_unix_seconds(at.unix_timestamp()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_becomes_what_a_listing_reports() {
        let resource: FileResource = serde_json::from_str(
            r#"{
                "id": "1a2B3c",
                "name": "jrn-1.cfrt",
                "size": "4096",
                "md5Checksum": "0cc175b9c0f1b6a831c399e269772661",
                "modifiedTime": "2026-08-19T12:34:56.789Z"
            }"#,
        )
        .expect("Drive's shape must parse");

        let info = resource.to_object_info();
        assert_eq!(info.object_ref.as_str(), "1a2B3c");
        assert_eq!(info.name, "jrn-1.cfrt");
        assert_eq!(info.size, 4096);
        assert_eq!(info.mtime.as_unix_seconds(), 1_787_142_896);
        assert_eq!(
            info.hash.map(|hash| hash.to_string()),
            Some("0cc175b9c0f1b6a831c399e269772661".to_owned())
        );
    }

    #[test]
    fn a_file_drive_says_little_about_still_describes_itself() {
        let resource: FileResource =
            serde_json::from_str(r#"{"id": "1a2B3c"}"#).expect("an id alone must parse");

        let info = resource.to_object_info();
        assert_eq!(info.size, 0);
        assert_eq!(info.hash, None);
    }
}
