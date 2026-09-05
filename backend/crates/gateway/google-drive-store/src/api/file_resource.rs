use coffret_usecase::{Error, ObjectInfo, ObjectRef, ProviderHash, Result};
use serde::Deserialize;

/// The fields the gateway asks Drive for about a file.
///
/// Drive returns a small default set unless told otherwise, so every call names
/// exactly what the port reports and nothing more.
pub const FILE_FIELDS: &str = "id,name,md5Checksum";

/// The fields a listing asks for, which is the file set plus the page marker.
pub const LIST_FIELDS: &str = "nextPageToken,files(id,name,md5Checksum)";

/// One file as Drive describes it.
///
/// Every field but the identifier is optional here because that is the shape
/// the answer arrives in, not because the gateway takes every absence for an
/// answer: [`to_object_info`](Self::to_object_info) is where they are told
/// apart.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileResource {
    /// Drive's identifier for the file, which is what an [`ObjectRef`] carries.
    pub id: String,
    /// The name the file was created under.
    pub name: Option<String>,
    /// Drive's MD5 of the stored bytes.
    pub md5_checksum: Option<String>,
}

impl FileResource {
    /// What the port reports about this file, or a refusal where Drive
    /// described it with something the port cannot report.
    ///
    /// # Errors
    ///
    /// [`Error::MalformedResponse`] where the file carries no `name` — a field
    /// [`LIST_FIELDS`] asks for by name, so its absence is not Drive leaving
    /// out something that was never requested — or where its `id` is the empty
    /// string, which would address the next call at the collection of files
    /// rather than at this one.
    ///
    /// The digest is not among them: Drive reports none for some of the files
    /// it holds, and a listing saying so is an answer (see [`ProviderHash`]).
    pub fn to_object_info(&self) -> Result<ObjectInfo> {
        // Both refusals name the entry they are about, so a page of a few
        // hundred files reported as one malformed answer still says which of
        // them it was: the identifier where there is no name, the name where
        // the identifier is the thing missing.
        let Some(name) = self.name.clone() else {
            return Err(Error::MalformedResponse {
                detail: format!("Storage listed the object {:?} without a name", self.id),
            });
        };
        if self.id.is_empty() {
            return Err(Error::MalformedResponse {
                detail: format!("Storage listed {name:?} under an empty identifier"),
            });
        }
        Ok(ObjectInfo {
            object_ref: ObjectRef::new(&self.id),
            name,
            hash: self.md5_checksum.as_deref().map(ProviderHash::new),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_becomes_what_a_listing_reports() {
        let resource: FileResource = serde_json::from_str(
            r#"{
                "id": "1a2B3c",
                "name": "head-1.cfrt",
                "md5Checksum": "0cc175b9c0f1b6a831c399e269772661"
            }"#,
        )
        .expect("Drive's shape must parse");

        let info = resource
            .to_object_info()
            .expect("a file Drive described in full must be reportable");
        assert_eq!(info.object_ref.as_str(), "1a2B3c");
        assert_eq!(info.name, "head-1.cfrt");
        assert_eq!(
            info.hash.map(|hash| hash.to_string()),
            Some("0cc175b9c0f1b6a831c399e269772661".to_owned())
        );
    }

    // A file Drive reports no digest for is still a file: the port keeps that
    // absence as `None` rather than reading it as a bad answer.
    #[test]
    fn a_file_drive_reports_no_digest_for_still_describes_itself() {
        let resource: FileResource =
            serde_json::from_str(r#"{"id": "1a2B3c", "name": "head-1.cfrt"}"#)
                .expect("a file without a digest must parse");

        let info = resource
            .to_object_info()
            .expect("a digest Drive does not report is not a malformed answer");
        assert_eq!(info.name, "head-1.cfrt");
        assert_eq!(info.hash, None);
    }

    #[test]
    fn a_listed_file_without_a_name_is_a_malformed_response() {
        let resource: FileResource =
            serde_json::from_str(r#"{"id": "1a2B3c"}"#).expect("an id alone must parse");

        assert!(matches!(
            resource.to_object_info(),
            Err(Error::MalformedResponse { .. })
        ));
    }

    #[test]
    fn a_listed_file_with_an_empty_id_is_a_malformed_response() {
        let resource: FileResource = serde_json::from_str(r#"{"id": "", "name": "head-1.cfrt"}"#)
            .expect("an empty id must parse");

        assert!(matches!(
            resource.to_object_info(),
            Err(Error::MalformedResponse { .. })
        ));
    }
}
