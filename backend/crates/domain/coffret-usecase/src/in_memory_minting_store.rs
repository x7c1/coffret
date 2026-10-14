use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Mutex;

use async_trait::async_trait;
use coffret_model::{lowercase_hex, ObjectRef};
use md5::{Digest, Md5};

use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::{Error, Result};
use crate::missing::Missing;
use crate::object_info::ObjectInfo;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::provider_hash::ProviderHash;
use crate::uploaded_object::UploadedObject;

/// An [`ObjectStore`] that keeps everything in memory and identifies objects by
/// identifiers it mints rather than by their names, for tests.
///
/// [`InMemoryStore`](crate::InMemoryStore) is modelled on the name-keyed
/// providers. This one is modelled on the other shape, Google Drive's, in the
/// three ways the commit protocol depends on:
///
/// - [`reserve_create`](ObjectStore::reserve_create) mints a fresh identifier
///   on every call, as `files.generateIds` does, so two writers that reserve
///   independently hold two different slots even for one name;
/// - [`put_if_absent`](ObjectStore::put_if_absent) excludes by that identifier
///   alone, and an identifier once used stays refused after its object is
///   purged, as Drive refuses a create under a burned pre-minted id;
/// - names are not unique: [`put`](ObjectStore::put) creates a new object every
///   time, and the listing returns every live object, two under one name
///   included.
///
/// So the exclusion a head's commit slot stands for holds here only as far as
/// every writer consumes the one identifier the head carries — which is the
/// thing a test against this store can prove and one against a name-keyed store
/// cannot (spec: CP-2, CP-3).
#[derive(Debug)]
pub struct InMemoryMintingStore {
    state: Mutex<State>,
    page_size: usize,
}

/// Everything the store holds, keyed by the identifier it minted.
#[derive(Debug, Default)]
struct State {
    live: BTreeMap<String, Stored>,
    trashed: BTreeMap<String, Stored>,
    /// Identifiers a create has used, which a later create may not use again.
    burned: BTreeSet<String>,
    minted: u64,
}

/// One object: the name it was created under, and its bytes.
#[derive(Debug, Clone)]
struct Stored {
    name: String,
    bytes: Vec<u8>,
}

impl State {
    /// A fresh identifier, never handed out before.
    fn mint(&mut self) -> String {
        self.minted += 1;
        format!("minted-{:08}", self.minted)
    }
}

impl InMemoryMintingStore {
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
            state: Mutex::new(State::default()),
            page_size,
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// Where a listing stands in this store's order: by name, then by identifier,
/// so two objects under one name are listed side by side and in a fixed order.
fn position(name: &str, id: &str) -> String {
    // A NUL sorts below every character a name or an identifier holds, so the
    // spelled position sorts exactly as the pair does.
    format!("{name}\u{0}{id}")
}

#[async_trait]
impl ObjectStore for InMemoryMintingStore {
    async fn put(&self, name: &str, body: ByteStream) -> Result<UploadedObject> {
        let bytes = body.into_bytes().await?;
        let hash = ProviderHash::new(digest(&bytes));
        let mut state = self.state();
        let id = state.mint();
        state.burned.insert(id.clone());
        state.live.insert(
            id.clone(),
            Stored {
                name: name.to_owned(),
                bytes,
            },
        );
        Ok(UploadedObject {
            object_ref: ObjectRef::new(id),
            hash: Some(hash),
        })
    }

    async fn reserve_create(&self, name: &str) -> Result<CommitSlot> {
        let id = self.state().mint();
        Ok(CommitSlot::provider_id(name, id))
    }

    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> Result<ObjectRef> {
        let id = slot.require_provider_id()?.to_owned();
        let bytes = body.into_bytes().await?;

        // Read and write under one lock: a check that released it before
        // writing would let both writers pass it.
        let mut state = self.state();
        if !state.burned.insert(id.clone()) {
            return Err(Error::AlreadyExists {
                object: slot.name().to_owned(),
            });
        }
        state.live.insert(
            id.clone(),
            Stored {
                name: slot.name().to_owned(),
                bytes,
            },
        );
        Ok(ObjectRef::new(id))
    }

    fn object_at(&self, slot: &CommitSlot) -> Result<ObjectRef> {
        Ok(ObjectRef::new(slot.require_provider_id()?))
    }

    async fn get(&self, object: &ObjectRef, range: Option<Range<u64>>) -> Result<ByteStream> {
        let id = object.as_str();
        let state = self.state();
        let bytes = &state
            .live
            .get(id)
            .or_else(|| state.trashed.get(id))
            .ok_or_else(|| Error::NotFound {
                missing: Missing::Object(id.to_owned()),
            })?
            .bytes;

        let bytes = match range {
            None => bytes.clone(),
            Some(range) if range.is_empty() => {
                return Err(Error::Unsupported {
                    detail: format!("an empty byte range asks for no bytes: {range:?}"),
                    source: None,
                })
            }
            Some(range) => {
                let start = usize::try_from(range.start).unwrap_or(usize::MAX);
                let end = usize::try_from(range.end)
                    .unwrap_or(usize::MAX)
                    .min(bytes.len());
                if start >= bytes.len() {
                    return Err(Error::Unsupported {
                        detail: format!("{range:?} starts past the end of {id:?}"),
                        source: None,
                    });
                }
                bytes[start..end].to_vec()
            }
        };
        Ok(ByteStream::from(bytes))
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        let state = self.state();
        let mut ordered: Vec<(String, &String, &Stored)> = state
            .live
            .iter()
            .map(|(id, stored)| (position(&stored.name, id), id, stored))
            .collect();
        ordered.sort_by(|left, right| left.0.cmp(&right.0));

        let mut remaining = ordered
            .into_iter()
            .skip_while(|(at, _, _)| page.is_some_and(|token| at.as_str() <= token.as_str()));
        let listed: Vec<(String, ObjectInfo)> = remaining
            .by_ref()
            .take(self.page_size)
            .map(|(at, id, stored)| {
                (
                    at,
                    ObjectInfo {
                        object_ref: ObjectRef::new(id),
                        name: stored.name.clone(),
                        hash: Some(ProviderHash::new(digest(&stored.bytes))),
                    },
                )
            })
            .collect();

        let next = match listed.last() {
            Some((at, _)) if remaining.next().is_some() => Some(PageToken::new(at.clone())),
            _ => None,
        };
        let objects = listed.into_iter().map(|(_, info)| info).collect();
        Ok(match next {
            Some(next) => ObjectPage::resumable(objects, next),
            None => ObjectPage::last(objects),
        })
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        let id = object.as_str();
        let mut state = self.state();
        if let Some(stored) = state.live.remove(id) {
            state.trashed.insert(id.to_owned(), stored);
        }
        Ok(())
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        let id = object.as_str();
        let mut state = self.state();
        state.live.remove(id);
        state.trashed.remove(id);
        Ok(())
    }
}

/// An MD5 of the stored bytes, as Drive reports one.
fn digest(bytes: &[u8]) -> String {
    let mut state = Md5::new();
    state.update(bytes);
    lowercase_hex::encode(&state.finalize().into())
}
