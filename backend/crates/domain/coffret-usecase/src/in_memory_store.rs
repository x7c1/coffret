use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Mutex;

use async_trait::async_trait;
use coffret_model::{Mtime, ObjectRef};
use md5::{Digest, Md5};

use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::{Error, Result};
use crate::object_info::ObjectInfo;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::provider_hash::ProviderHash;

/// An [`ObjectStore`] that keeps everything in memory, for tests.
///
/// It is modelled on the name-keyed providers rather than on the ones that mint
/// identifiers, because that is the shape the commit protocol is hardest on: a
/// slot is nothing but a name, so a conditional create is exclusive only as far
/// as two writers derive the same name. A test that proves an exclusion here has
/// proved it where it can actually fail.
///
/// Trash is modelled the way S3's adapter models it — a second key space the
/// listing does not reach — so a trashed object stays readable through the
/// reference that named it, and purging reaches it either way.
///
/// It lives in the crate the port lives in, not in a gateway, because what it
/// implements is the port's contract and nothing about any provider.
#[derive(Debug)]
pub struct InMemoryStore {
    objects: Mutex<Objects>,
    page_size: usize,
}

/// What the store holds, live and trashed.
#[derive(Debug, Default)]
struct Objects {
    live: BTreeMap<String, Vec<u8>>,
    trashed: BTreeMap<String, Vec<u8>>,
}

impl InMemoryStore {
    /// An empty store whose listing pages hold `page_size` objects.
    ///
    /// # Panics
    ///
    /// If `page_size` is zero: a listing whose pages hold nothing never
    /// finishes.
    pub fn new(page_size: usize) -> Self {
        assert!(
            page_size > 0,
            "a listing page must hold at least one object"
        );
        Self {
            objects: Mutex::new(Objects::default()),
            page_size,
        }
    }

    /// How many objects one listing page of this store holds.
    pub fn page_size(&self) -> usize {
        self.page_size
    }

    fn objects(&self) -> std::sync::MutexGuard<'_, Objects> {
        self.objects
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[async_trait]
impl ObjectStore for InMemoryStore {
    async fn put(&self, name: &str, body: ByteStream) -> Result<ObjectRef> {
        let bytes = body.into_bytes().await?;
        self.objects().live.insert(name.to_owned(), bytes);
        Ok(ObjectRef::new(name))
    }

    async fn reserve_create(&self, name: &str) -> Result<CommitSlot> {
        // The key space is the slot space: an object's name already says where
        // it would go, so there is nothing to allocate. Reserving one name
        // twice therefore yields the same slot, and the race is settled by the
        // conditional create itself.
        Ok(CommitSlot::by_name(name))
    }

    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> Result<ObjectRef> {
        let name = slot.require_name()?;
        let bytes = body.into_bytes().await?;

        // Read and write under one lock: a check that released it before
        // writing would let both writers pass it.
        let mut objects = self.objects();
        if objects.live.contains_key(name) {
            return Err(Error::AlreadyExists {
                object: name.to_owned(),
            });
        }
        objects.live.insert(name.to_owned(), bytes);
        Ok(ObjectRef::new(name))
    }

    fn object_at(&self, slot: &CommitSlot) -> Result<ObjectRef> {
        // The name is the handle here, so a slot names its object whether or
        // not anything has been written into it yet.
        Ok(ObjectRef::new(slot.require_name()?))
    }

    async fn get(&self, object: &ObjectRef, range: Option<Range<u64>>) -> Result<ByteStream> {
        let name = object.as_str();
        let objects = self.objects();
        let bytes = objects
            .live
            .get(name)
            .or_else(|| objects.trashed.get(name))
            .ok_or_else(|| Error::NotFound {
                object: name.to_owned(),
            })?;

        let bytes = match range {
            None => bytes.clone(),
            Some(range) if range.is_empty() => {
                return Err(Error::Unsupported {
                    detail: format!("an empty byte range asks for no bytes: {range:?}"),
                })
            }
            Some(range) => {
                let start = usize::try_from(range.start).unwrap_or(usize::MAX);
                let end = usize::try_from(range.end)
                    .unwrap_or(usize::MAX)
                    .min(bytes.len());
                if start >= bytes.len() {
                    return Err(Error::Unsupported {
                        detail: format!("{range:?} starts past the end of {name:?}"),
                    });
                }
                bytes[start..end].to_vec()
            }
        };
        Ok(ByteStream::from(bytes))
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        let objects = self.objects();
        // The token is the name the previous page ended on, so a walk resumes
        // at whatever now follows it rather than at a remembered offset.
        let mut remaining = objects.live.iter().skip_while(|(name, _)| match page {
            Some(token) => name.as_str() <= token.as_str(),
            None => false,
        });

        let listed: Vec<ObjectInfo> = remaining
            .by_ref()
            .take(self.page_size)
            .map(|(name, bytes)| ObjectInfo {
                object_ref: ObjectRef::new(name),
                name: name.clone(),
                size: bytes.len() as u64,
                mtime: Mtime::from_unix_seconds(0),
                hash: Some(ProviderHash::new(digest(bytes))),
            })
            .collect();

        match listed.last() {
            // A page that filled up may still be the last one, so the token is
            // only issued when something actually follows it.
            Some(last) if remaining.next().is_some() => {
                let next = PageToken::new(last.name.clone());
                Ok(ObjectPage::resumable(listed, next))
            }
            _ => Ok(ObjectPage::last(listed)),
        }
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        let name = object.as_str();
        let mut objects = self.objects();
        if let Some(bytes) = objects.live.remove(name) {
            objects.trashed.insert(name.to_owned(), bytes);
        }
        Ok(())
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        let name = object.as_str();
        let mut objects = self.objects();
        objects.live.remove(name);
        objects.trashed.remove(name);
        Ok(())
    }
}

/// A digest of the stored bytes, in this store's own spelling.
///
/// A [`ProviderHash`] is provider-scoped by definition, so a token of this
/// store's own invention would satisfy the port. It would not satisfy the
/// callers: an uploader compares what it hashed on the way out against what the
/// provider reports for what it stored, and both providers coffret has an
/// adapter for report an MD5 — Drive by name, S3 as the ETag of a
/// single-request upload. A reference store whose digest nothing computed
/// locally could be compared against would leave every such caller untestable
/// in memory, so this reports the digest they do.
fn digest(bytes: &[u8]) -> String {
    let mut state = Md5::new();
    state.update(bytes);
    state
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
