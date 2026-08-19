use std::ops::Range;
use std::sync::Arc;

use async_trait::async_trait;
use coffret_usecase::{
    ByteStream, CommitSlot, Error, ObjectPage, ObjectRef, ObjectStore, PageToken, Result,
};
use serde_json::json;

use crate::api::{
    authorization, live_files_query, DriveApi, Endpoints, FailedResponse, FileList, FileResource,
    GeneratedIds, LIST_FIELDS,
};
use crate::http::{HttpRequest, HttpResponse, HttpTransport, Method};
use crate::oauth::AccessTokens;
use crate::settings::DriveSettings;
use crate::upload;

/// A Library kept in a Google Drive folder.
///
/// Drive names files by an identifier it mints rather than by their name, which
/// is what makes the commit slot real here: `files.generateIds` reserves an
/// identifier, and the create that names it either lands or finds it taken.
/// Both halves of the port's removal are Drive's own — its trash and its
/// permanent delete — so nothing has to be simulated.
///
/// The grant asked for is `drive.file` and nothing else, so this reaches only
/// the files coffret itself created. The transport and the token source are
/// constructor arguments, which is what lets the retry and integrity behaviour
/// be tested without a Google account.
pub struct GoogleDrive {
    api: DriveApi,
    settings: DriveSettings,
}

impl GoogleDrive {
    /// Takes a transport, a source of access tokens, and the folder to work in.
    pub fn new(
        transport: Arc<dyn HttpTransport>,
        tokens: Arc<dyn AccessTokens>,
        settings: DriveSettings,
    ) -> Self {
        Self {
            api: DriveApi::new(transport, tokens, Endpoints::default()),
            settings,
        }
    }

    /// The metadata a create carries: where the file goes, and what it is called.
    fn metadata(&self, name: &str, id: Option<&str>) -> serde_json::Value {
        let mut metadata = json!({
            "name": name,
            "parents": [self.settings.folder_id()],
        });
        if let Some(id) = id {
            metadata["id"] = json!(id);
        }
        metadata
    }

    /// Reads a JSON answer, or turns a refusal into one of the port's errors.
    async fn read_json<T: serde::de::DeserializeOwned>(
        response: HttpResponse,
        object: &str,
    ) -> Result<T> {
        if !response.is_success() {
            return Err(FailedResponse::read(response).await.into_error(object));
        }

        let body = response.into_body().into_bytes().await?;
        serde_json::from_slice(&body).map_err(|error| Error::MalformedResponse {
            detail: format!("unreadable answer for {object:?}: {error}"),
        })
    }

    /// Whether a file is still there.
    async fn exists(&self, id: &str) -> Result<bool> {
        let url = format!("{}?fields=id", self.api.endpoints().file(id));
        let response = self
            .api
            .send(|token| {
                let (header, value) = authorization(token);
                HttpRequest::new(Method::Get, &url).with_header(header, value)
            })
            .await?;

        if response.status() == 404 {
            return Ok(false);
        }
        if !response.is_success() {
            return Err(FailedResponse::read(response).await.into_error(id));
        }
        Ok(true)
    }
}

/// Checks a name is one Drive can store.
fn validate(name: &str) -> Result<()> {
    if name.is_empty() {
        return Err(Error::Unsupported {
            detail: "an object name cannot be empty".to_owned(),
        });
    }
    Ok(())
}

/// The `Range` header for a half-open byte range.
///
/// HTTP ranges are inclusive at both ends, so the last byte asked for is one
/// before the end of the range.
fn range_header(range: &Range<u64>) -> Result<String> {
    if range.is_empty() {
        return Err(Error::Unsupported {
            detail: format!("an empty byte range asks for no bytes: {range:?}"),
        });
    }
    Ok(format!("bytes={}-{}", range.start, range.end - 1))
}

#[async_trait]
impl ObjectStore for GoogleDrive {
    async fn put(&self, name: &str, body: ByteStream) -> Result<ObjectRef> {
        validate(name)?;
        upload::create(
            &self.api,
            name,
            self.metadata(name, None),
            body,
            FailedResponse::into_error,
        )
        .await
    }

    async fn reserve_create(&self) -> Result<CommitSlot> {
        let url = format!(
            "{}/generateIds?count=1&space=drive",
            self.api.endpoints().files()
        );

        let response = self
            .api
            .send(|token| {
                let (header, value) = authorization(token);
                HttpRequest::new(Method::Get, &url).with_header(header, value)
            })
            .await?;

        let generated: GeneratedIds = Self::read_json(response, "a commit slot").await?;
        let id = generated
            .ids
            .into_iter()
            .next()
            .ok_or_else(|| Error::MalformedResponse {
                detail: "Storage minted no identifier for the commit slot".to_owned(),
            })?;

        Ok(CommitSlot::provider_id(id))
    }

    async fn put_if_absent(
        &self,
        slot: &CommitSlot,
        name: &str,
        body: ByteStream,
    ) -> Result<ObjectRef> {
        let Some(id) = slot.as_provider_id() else {
            return Err(Error::Unsupported {
                detail: "this store creates objects under a minted id, not by name".to_owned(),
            });
        };
        validate(name)?;

        upload::create(
            &self.api,
            name,
            self.metadata(name, Some(id)),
            body,
            FailedResponse::into_conditional_create_error,
        )
        .await
    }

    async fn get(&self, object: &ObjectRef, range: Option<Range<u64>>) -> Result<ByteStream> {
        let id = object.as_str();
        let url = format!("{}?alt=media", self.api.endpoints().file(id));
        let range = match &range {
            Some(range) => Some(range_header(range)?),
            None => None,
        };

        let response = self
            .api
            .send(|token| {
                let (header, value) = authorization(token);
                let request = HttpRequest::new(Method::Get, &url).with_header(header, value);
                match &range {
                    Some(range) => request.with_header("range", range),
                    None => request,
                }
            })
            .await?;

        if !response.is_success() {
            return Err(FailedResponse::read(response).await.into_error(id));
        }
        Ok(response.into_body())
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        // Built and finished in one go: the query builder borrows, and holding
        // it across the call below would tie the request's lifetime to it.
        let url = {
            let mut query = url::form_urlencoded::Serializer::new(String::new());
            let page_size = self.settings.page_size().to_string();
            query.extend_pairs([
                ("q", live_files_query(self.settings.folder_id()).as_str()),
                ("fields", LIST_FIELDS),
                ("pageSize", page_size.as_str()),
                // Ordering is not part of the port's contract, but a stable one
                // is what keeps a page boundary from moving under a walk.
                ("orderBy", "name"),
            ]);
            if let Some(token) = page {
                query.append_pair("pageToken", token.as_str());
            }
            format!("{}?{}", self.api.endpoints().files(), query.finish())
        };

        let response = self
            .api
            .send(|token| {
                let (header, value) = authorization(token);
                HttpRequest::new(Method::Get, &url).with_header(header, value)
            })
            .await?;

        let listing: FileList = Self::read_json(response, "a listing").await?;
        let objects = listing
            .files
            .iter()
            .map(FileResource::to_object_info)
            .collect();

        Ok(match listing.next_page_token {
            Some(token) => ObjectPage::resumable(objects, PageToken::new(token)),
            None => ObjectPage::last(objects),
        })
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        let id = object.as_str();
        let url = format!("{}?fields=id", self.api.endpoints().file(id));
        let trashed = json!({ "trashed": true });

        let response = self
            .api
            .send(|token| {
                let (header, value) = authorization(token);
                HttpRequest::new(Method::Patch, &url)
                    .with_header(header, value)
                    .with_json(&trashed)
            })
            .await?;

        if response.is_success() {
            Ok(())
        } else {
            Err(FailedResponse::read(response).await.into_error(id))
        }
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        let id = object.as_str();
        let url = self.api.endpoints().file(id);

        let response = self
            .api
            .send(|token| {
                let (header, value) = authorization(token);
                HttpRequest::new(Method::Delete, &url).with_header(header, value)
            })
            .await?;

        // Deleting something already gone is what an interrupted rotation does
        // when it is run again, and it has to be a no-op rather than an error
        // that stalls the retry.
        if !response.is_success() && response.status() != 404 {
            return Err(FailedResponse::read(response).await.into_error(id));
        }

        // Read back: a rotation is only complete once the old-epoch objects are
        // really gone, so an unconfirmed deletion is a failure.
        if self.exists(id).await? {
            return Err(Error::NotPurged {
                object: id.to_owned(),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_half_open_range_becomes_an_inclusive_header() {
        assert_eq!(range_header(&(10..20)).unwrap(), "bytes=10-19");
    }

    #[test]
    fn an_empty_range_is_refused_rather_than_sent() {
        assert!(matches!(
            range_header(&(10..10)),
            Err(Error::Unsupported { .. })
        ));
    }
}
