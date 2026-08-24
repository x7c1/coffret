use std::ops::Range;
use std::sync::Mutex;

use async_trait::async_trait;
use coffret_model::ObjectRef;

use crate::byte_stream::ByteStream;
use crate::commit::{commit_batch, CommitRequest, ControlKeys, PreparedBatch};
use crate::commit_conformance::fixtures::policy;
use crate::commit_conformance::is_head;
use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::index::Index;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;

/// A store that lets another device commit first, exactly once.
///
/// Two commits started together do not reliably collide. Whether they do
/// depends on how the runtime interleaves them and on how long Storage takes to
/// answer, so a suite that only started two writers at once would be testing the
/// rebase on some backends and not on others — and passing either way.
///
/// This puts the collision where it can be asserted: the rival commits at the
/// moment the writer under test reaches the conditional create of its record,
/// so the create is refused by a head that really is there, holding a record
/// another device really wrote (spec: CP-3, CP-4). The rival goes through the
/// same [`commit_batch`] the case is testing, because a hand-built object in the
/// slot would be a rebase onto something no commit produced.
pub(super) struct RacingStore<'a> {
    inner: &'a dyn ObjectStore,
    rival: Mutex<Option<Rival<'a>>>,
}

/// The other device, and what it commits when its moment comes.
struct Rival<'a> {
    index: &'a dyn Index,
    keys: &'a ControlKeys,
    batch: PreparedBatch,
}

impl<'a> RacingStore<'a> {
    /// Wraps `inner` so that `index` commits `batch` just before the next
    /// create aimed at the control-head chain.
    pub(super) fn letting_in(
        inner: &'a dyn ObjectStore,
        index: &'a dyn Index,
        keys: &'a ControlKeys,
        batch: PreparedBatch,
    ) -> Self {
        Self {
            inner,
            rival: Mutex::new(Some(Rival { index, keys, batch })),
        }
    }

    /// The rival's commit, if it has not had its turn yet.
    fn take_rival(&self) -> Option<Rival<'a>> {
        self.rival
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }
}

#[async_trait]
impl ObjectStore for RacingStore<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<ObjectRef> {
        self.inner.put(name, body).await
    }

    async fn reserve_create(&self, name: &str) -> Result<CommitSlot> {
        self.inner.reserve_create(name).await
    }

    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> Result<ObjectRef> {
        if is_head(slot.name()) {
            // The lock is released before the rival runs: it commits through
            // this same store's inner one, and holding a guard across that
            // would deadlock rather than test anything.
            if let Some(rival) = self.take_rival() {
                commit_batch(
                    CommitRequest::new(self.inner, rival.index, rival.keys, rival.batch)
                        .with_policy(policy()),
                )
                .await
                .expect("the rival commits uncontested and must succeed");
            }
        }
        self.inner.put_if_absent(slot, body).await
    }

    fn object_at(&self, slot: &CommitSlot) -> Result<ObjectRef> {
        self.inner.object_at(slot)
    }

    async fn get(&self, object: &ObjectRef, range: Option<Range<u64>>) -> Result<ByteStream> {
        self.inner.get(object, range).await
    }

    async fn list(&self, page: Option<&PageToken>) -> Result<ObjectPage> {
        self.inner.list(page).await
    }

    async fn trash(&self, object: &ObjectRef) -> Result<()> {
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.inner.purge(object).await
    }
}
