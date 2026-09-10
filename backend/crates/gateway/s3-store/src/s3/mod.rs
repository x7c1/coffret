use std::ops::Range;

use async_trait::async_trait;
use aws_sdk_s3::Client;
use coffret_logging::redact::PrivateValues;
use coffret_usecase::{
    ByteStream, CommitSlot, Error, ObjectPage, ObjectRef, ObjectStore, PageToken, Result,
};
use tracing::{debug, info, warn};

use crate::error::{
    is_not_found, translate_conditional_create, translate_listing, translate_object,
};
use crate::key_layout::{KeyLayout, DELIMITER};
use crate::reader_body::to_sdk_stream;
use crate::settings::S3Settings;
use crate::single_request_limit::refuse_oversized;

mod listed_object;
use listed_object::describe;

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
    private: PrivateValues,
}

impl S3 {
    /// Takes a configured client and the Library's place in a bucket.
    pub fn new(client: Client, settings: S3Settings) -> Self {
        let layout = KeyLayout::new(settings.prefix());
        // Where this Library lives is somebody's arrangement rather than
        // anything coffret minted, and S3 answers a refusal by quoting what it
        // was asked for — bucket and key together — so both halves of that
        // arrangement are named once here and taken out of every diagnostic
        // this gateway records (spec: EL-5). The prefix goes in without its
        // trailing separator, which is the one spelling both forms of it
        // contain: `LibraryId::app_prefix` ends every prefix it builds with a
        // separator (spec: FM-18), and S3 quotes back the whole key it was
        // asked for — so a value ending in `/` would stand next to the object
        // name rather than next to a boundary, and a value that never stands as
        // a token is one that stays in the log. Trimmed, it is bounded by the
        // separator that follows it, in either spelling.
        let private = PrivateValues::none()
            .with(settings.bucket())
            .with(settings.prefix().trim_end_matches(DELIMITER));
        Self {
            client,
            settings,
            layout,
            private,
        }
    }

    /// Whether the key an object of this name is stored under holds anything.
    ///
    /// The name travels alongside the key it was turned into, as it does for
    /// [`Self::delete`]: a key begins with the prefix the Library was
    /// configured into, and what a refusal reports and an event records is the
    /// name coffret minted rather than that location (spec: EL-5).
    async fn exists(&self, operation: &'static str, name: &str, key: &str) -> Result<bool> {
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
            Err(error) => return Err(translate_object(operation, name, error, &self.private)),
        };

        answered(operation, "head_object", name);
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
            .map_err(|error| translate_object(operation, name, error, &self.private))?;

        answered(operation, "delete_object", name);
        Ok(())
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
/// What is recorded is the object's name and not the key it is stored under:
/// the name is one coffret minted, while the key begins with the prefix the
/// Library was configured into — somebody's configuration rather than anything
/// this Library minted, which the same field of a [`translate`] event leaves
/// out for the same reason (spec: EL-5). A listing addresses no object at
/// all — the prefix would be the whole of the field — so it is recorded by
/// [`answered_listing`] instead.
fn answered(operation: &'static str, call: &'static str, object: &str) {
    debug!(operation, call, object, "Storage answered a call");
}

/// Records a successful listing without its configured location.
fn answered_listing() {
    debug!(
        operation = "list",
        call = "list_objects_v2",
        "Storage answered a call"
    );
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
        // The stream says how long it is before it is read, which is what makes
        // this answerable now rather than after several gigabytes have gone up.
        refuse_oversized(len)?;
        let key = self.layout.live_key(name);

        self.client
            .put_object()
            .bucket(self.settings.bucket())
            .key(&key)
            .content_length(len as i64)
            .body(to_sdk_stream(body))
            .send()
            .await
            .map_err(|error| translate_object("put", name, error, &self.private))?;

        answered("put", "put_object", name);
        // Ordinary progress: what went up, and how much of it. The name is one
        // coffret minted and the size is of ciphertext, so neither names a file
        // or a location. The key it went under is not what is recorded, for the
        // reason `answered` gives.
        info!(
            operation = "put",
            object = name,
            bytes = len,
            "stored an object"
        );
        Ok(ObjectRef::new(name))
    }

    async fn reserve_create(&self, name: &str) -> Result<CommitSlot> {
        // The key space is the slot space: an object's name already says where
        // it would go, so there is nothing to allocate and nothing that could
        // fail beyond the name itself. Reserving one name twice therefore
        // yields the same slot, and the race is settled by the conditional PUT.
        self.layout.validate(name)?;
        Ok(CommitSlot::by_name(name))
    }

    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> Result<ObjectRef> {
        let name = slot.require_name()?;
        self.layout.validate(name)?;
        let len = body.len();
        refuse_oversized(len)?;
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
            .map_err(|error| {
                translate_conditional_create("put_if_absent", name, error, &self.private)
            })?;

        answered("put_if_absent", "put_object", name);
        info!(
            operation = "put_if_absent",
            object = name,
            bytes = len,
            "stored an object"
        );
        Ok(ObjectRef::new(name))
    }

    fn object_at(&self, slot: &CommitSlot) -> Result<ObjectRef> {
        // The key is the handle here, so a slot names its object whether or not
        // anything has been written into it yet.
        Ok(ObjectRef::new(slot.require_name()?))
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
            .map_err(|error| translate_object("get", name, error, &self.private))?;

        answered("get", "get_object", name);
        // S3 states the length of every `GetObject` body it answers with, so an
        // answer that states none is not a body of no bytes: it is an answer
        // this build cannot read the object out of. Handing zero on would make
        // the first byte that arrived read as a stream overrunning what was
        // declared, which names the wrong thing entirely.
        let declared = response
            .content_length()
            .ok_or_else(|| Error::MalformedResponse {
                detail: format!("Storage answered the read of {name:?} with no content length"),
            })?;
        let len = u64::try_from(declared).map_err(|_| Error::MalformedResponse {
            detail: format!(
                "Storage answered the read of {name:?} with a content length of {declared}, \
                 which is not a count of bytes"
            ),
        })?;
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
            .map_err(|error| translate_listing(error, &self.private))?;

        answered_listing();
        // A listing that names one object this build cannot read refuses the
        // whole page rather than reporting the rest of it: see `describe`.
        let objects = response
            .contents()
            .iter()
            .filter_map(|object| describe(&self.layout, object).transpose())
            .collect::<Result<Vec<_>>>()?;

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
            .map_err(|error| translate_object("trash", name, error, &self.private))?;

        answered("trash", "copy_object", name);
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
        // really gone, so an unconfirmed deletion is a failure. Both halves are
        // read even when the first one still holds something, so that the event
        // below can say which of them a deletion did not take on; the cost is
        // one extra HEAD on a path that has already stopped a rotation. A probe
        // that fails on its own account is reported as that failure, even where
        // the other half has already answered that it kept the object: purge is
        // idempotent, so a retryable failure sends the caller round again and
        // the next read back settles both halves at once, where `NotPurged`
        // would state as settled a deletion only half of which was checked.
        let live_remains = self.exists("purge", name, &live).await?;
        let trashed_remains = self.exists("purge", name, &trashed).await?;
        if live_remains || trashed_remains {
            // The only account of a stopped rotation there is: `NotPurged`
            // reaches its caller as the object's name and nothing else, and
            // the calls above are recorded by that one name, which both halves
            // of the key space share. Which half an object was left in is this
            // gateway's own layout rather than anything the person configured,
            // so it is evidence an event may keep (spec: EL-5).
            warn!(
                operation = "purge",
                object = name,
                live = live_remains,
                trashed = trashed_remains,
                "an object was still in Storage after being purged"
            );
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
