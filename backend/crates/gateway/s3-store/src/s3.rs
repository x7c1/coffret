use std::ops::Range;

use async_trait::async_trait;
use aws_sdk_s3::Client;
use coffret_model::Mtime;
use coffret_usecase::{
    ByteStream, CommitSlot, Error, ObjectInfo, ObjectPage, ObjectRef, ObjectStore, PageToken,
    ProviderHash, Result,
};
use tracing::{debug, info};

use crate::error::{is_not_found, translate, translate_conditional_create};
use crate::key_layout::{KeyLayout, DELIMITER};
use crate::reader_body::to_sdk_stream;
use crate::settings::S3Settings;

/// A Library kept in an S3 bucket.
///
/// The object name is the key, so nothing has to be allocated before a
/// conditional create and an [`ObjectRef`] is simply the name. What S3 does not
/// have is a trash, so [`ObjectStore::trash`] makes one out of the key space —
/// live objects sit directly under the configured prefix and trashed ones under
/// a reserved `trash/` segment of it — and [`ObjectStore::purge`] clears an
/// object out of both halves.
///
/// The client is handed in rather than built here: region, credentials, and
/// endpoint are the caller's to decide, which is what lets the same gateway
/// serve AWS and a MinIO container without knowing the difference.
#[derive(Debug, Clone)]
pub struct S3 {
    client: Client,
    settings: S3Settings,
    layout: KeyLayout,
}

impl S3 {
    /// Takes a configured client and the Library's place in a bucket.
    pub fn new(client: Client, settings: S3Settings) -> Self {
        let layout = KeyLayout::new(settings.prefix());
        Self {
            client,
            settings,
            layout,
        }
    }

    /// Whether a key currently holds an object.
    async fn exists(&self, operation: &'static str, key: &str) -> Result<bool> {
        let found = match self
            .client
            .head_object()
            .bucket(self.settings.bucket())
            .key(key)
            .send()
            .await
        {
            Ok(_) => true,
            Err(error) if is_not_found(&error) => false,
            // Recorded by `translate` with the status S3 refused with, and
            // nothing answered, so there is no call to record as answered.
            Err(error) => return Err(translate(operation, key, error)),
        };

        answered(operation, "head_object", key);
        Ok(found)
    }

    /// Deletes a key, whether or not anything is stored under it.
    async fn delete(&self, operation: &'static str, name: &str, key: &str) -> Result<()> {
        self.client
            .delete_object()
            .bucket(self.settings.bucket())
            .key(key)
            .send()
            .await
            .map_err(|error| translate(operation, name, error))?;

        answered(operation, "delete_object", key);
        Ok(())
    }

    /// Turns one entry of a listing into what the port reports.
    ///
    /// Anything that is not a live object of this Library is skipped: asking S3
    /// to collapse keys past a separator already keeps the trash out, and this
    /// keeps a stray key someone else wrote under the prefix from being
    /// reported as a Storage Object.
    fn describe(&self, object: &aws_sdk_s3::types::Object) -> Option<ObjectInfo> {
        let name = self.layout.name_of(object.key()?)?;
        Some(ObjectInfo {
            object_ref: ObjectRef::new(name),
            name: name.to_owned(),
            size: object.size().unwrap_or_default().max(0) as u64,
            mtime: Mtime::from_unix_seconds(
                object
                    .last_modified()
                    .map(|at| at.secs())
                    .unwrap_or_default(),
            ),
            // S3 quotes its ETags; the quotes are transport syntax, not part of
            // the digest, and leaving them in would make the value fail to
            // compare against anything computed locally.
            hash: object
                .e_tag()
                .map(|tag| ProviderHash::new(tag.trim_matches('"'))),
        })
    }
}

/// Records that one call to S3 was made and came back.
///
/// What a single call did is ordinary detail, so it is `debug`: enough to
/// reconstruct what a run did against a provider, and too much to keep for
/// every run.
///
/// Less is recorded here than for a gateway that owns its own HTTP. The SDK
/// makes the request, and a successful output carries no status back out of it,
/// so there is no status on this event — inventing a field for a value this
/// crate does not have would make the log look like it answered a question it
/// cannot. A call that *failed* is recorded by [`translate`] instead, which does
/// have the status and the body S3 refused with.
///
/// The key is the prefix the call addressed where the call was a listing.
/// Nothing on the event is a credential: a key is a name coffret minted, and
/// the other two fields are constants.
fn answered(operation: &'static str, call: &'static str, key: &str) {
    debug!(operation, call, key, "Storage answered a call");
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
impl ObjectStore for S3 {
    async fn put(&self, name: &str, body: ByteStream) -> Result<ObjectRef> {
        self.layout.validate(name)?;
        let len = body.len();
        let key = self.layout.live_key(name);

        self.client
            .put_object()
            .bucket(self.settings.bucket())
            .key(&key)
            .content_length(len as i64)
            .body(to_sdk_stream(body))
            .send()
            .await
            .map_err(|error| translate("put", name, error))?;

        answered("put", "put_object", &key);
        // Ordinary progress: what went up, and how much of it. The key is
        // opaque and the size is of ciphertext, so neither says anything about
        // what was stored.
        info!(
            operation = "put",
            object = name,
            bytes = len,
            "stored an object"
        );
        Ok(ObjectRef::new(name))
    }

    async fn reserve_create(&self) -> Result<CommitSlot> {
        // The key space is the slot space: an object's name already says where
        // it would go, so there is nothing to allocate and nothing that could
        // fail. The race is settled by the conditional PUT itself.
        Ok(CommitSlot::by_name())
    }

    async fn put_if_absent(
        &self,
        slot: &CommitSlot,
        name: &str,
        body: ByteStream,
    ) -> Result<ObjectRef> {
        if let Some(id) = slot.as_provider_id() {
            return Err(Error::Unsupported {
                detail: format!("this store keys objects by name, not by minted id {id:?}"),
            });
        }
        self.layout.validate(name)?;
        let len = body.len();
        let key = self.layout.live_key(name);

        self.client
            .put_object()
            .bucket(self.settings.bucket())
            .key(&key)
            .content_length(len as i64)
            // "only if no object matches any entity tag" — that is, only if
            // nothing is stored under this key at all.
            .if_none_match("*")
            .body(to_sdk_stream(body))
            .send()
            .await
            .map_err(|error| translate_conditional_create("put_if_absent", name, error))?;

        answered("put_if_absent", "put_object", &key);
        info!(
            operation = "put_if_absent",
            object = name,
            bytes = len,
            "stored an object"
        );
        Ok(ObjectRef::new(name))
    }

    async fn get(&self, object: &ObjectRef, range: Option<Range<u64>>) -> Result<ByteStream> {
        let name = object.as_str();
        self.layout.validate(name)?;

        let key = self.layout.live_key(name);
        let mut request = self
            .client
            .get_object()
            .bucket(self.settings.bucket())
            .key(&key);

        if let Some(range) = &range {
            request = request.range(range_header(range)?);
        }

        let response = request
            .send()
            .await
            .map_err(|error| translate("get", name, error))?;

        answered("get", "get_object", &key);
        let len = response.content_length().unwrap_or_default().max(0) as u64;
        Ok(ByteStream::new(len, response.body.into_async_read()))
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        let mut request = self
            .client
            .list_objects_v2()
            .bucket(self.settings.bucket())
            .prefix(self.layout.live_prefix())
            // Collapse everything below a separator, which is what keeps the
            // trash out of the listing.
            .delimiter(DELIMITER)
            .max_keys(self.settings.page_size());

        if let Some(token) = page {
            request = request.continuation_token(token.as_str());
        }

        let response = request
            .send()
            .await
            .map_err(|error| translate("list", self.layout.live_prefix(), error))?;

        answered("list", "list_objects_v2", self.layout.live_prefix());
        let objects = response
            .contents()
            .iter()
            .filter_map(|object| self.describe(object))
            .collect();

        Ok(match response.next_continuation_token() {
            Some(token) => ObjectPage::resumable(objects, PageToken::new(token)),
            None => ObjectPage::last(objects),
        })
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        let name = object.as_str();
        self.layout.validate(name)?;

        let live = self.layout.live_key(name);
        let trashed = self.layout.trashed_key(name);

        // Copy first, delete second: the reverse order loses the object if the
        // second call fails, while this one at worst leaves a copy in the trash
        // that the next trash of the same name overwrites.
        self.client
            .copy_object()
            .bucket(self.settings.bucket())
            .key(&trashed)
            .copy_source(format!("{}/{}", self.settings.bucket(), live))
            .send()
            .await
            .map_err(|error| translate("trash", name, error))?;

        answered("trash", "copy_object", &trashed);
        self.delete("trash", name, &live).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        let name = object.as_str();
        self.layout.validate(name)?;

        let live = self.layout.live_key(name);
        let trashed = self.layout.trashed_key(name);

        // Both halves of the key space, because purge has to reach an object
        // whether or not it was trashed first, and because deleting a key that
        // holds nothing is a no-op in S3 — which is what makes repeating an
        // interrupted rotation safe.
        self.delete("purge", name, &live).await?;
        self.delete("purge", name, &trashed).await?;

        // Read back: a rotation is only complete once the old-epoch objects are
        // really gone, so an unconfirmed deletion is a failure.
        if self.exists("purge", &live).await? || self.exists("purge", &trashed).await? {
            return Err(Error::NotPurged {
                object: name.to_owned(),
            });
        }

        // Irreversible, and the step a Master Key rotation is judged on, so the
        // fact that it happened is ordinary progress worth keeping.
        info!(operation = "purge", object = name, "purged an object");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_half_open_range_becomes_an_inclusive_header() {
        assert_eq!(range_header(&(10..20)).unwrap(), "bytes=10-19");
        assert_eq!(range_header(&(0..1)).unwrap(), "bytes=0-0");
    }

    #[test]
    fn an_empty_range_is_refused_rather_than_sent() {
        assert!(matches!(
            range_header(&(10..10)),
            Err(Error::Unsupported { .. })
        ));
    }
}
