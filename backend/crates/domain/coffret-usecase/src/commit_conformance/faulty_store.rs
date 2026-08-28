use std::ops::Range;

use async_trait::async_trait;
use coffret_model::{ControlObjectName, ObjectRef};

use crate::byte_stream::ByteStream;
use crate::commit_conformance::is_head;
use crate::commit_slot::CommitSlot;
use crate::error::{Error, Result};
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;

/// A store that does one thing wrong, wrapped around the real one.
///
/// Four of the cases are about what a commit does when Storage does not
/// cooperate, and none of those states can be reached by driving the flow: a
/// replica that never arrives, a head that refuses the create, a snapshot slot
/// another device got to first, a provider that will not move anything to the
/// trash. Setting them up by hand afterwards would not be the same test — what
/// is being checked is where the flow *stops*, or that it declines to, which
/// only shows if the fault happens while it is running.
///
/// It wraps whatever store the backend handed the suite, so a case runs against
/// a real provider exactly as it runs in memory.
pub(super) struct FaultyStore<'a> {
    inner: &'a dyn ObjectStore,
    fault: Fault,
}

/// What the wrapper does differently.
enum Fault {
    /// Answers the write of one Keyring replica without storing anything.
    ///
    /// A provider that acknowledged a write and lost the object: the flow reads
    /// every replica back precisely because that is possible (spec: KL-2, CP-8).
    SwallowReplica(u16),
    /// Refuses the conditional create of a control head, permanently.
    ///
    /// The batch's Keyring candidate is already on Storage when this lands, so
    /// what is left behind is exactly an interrupted commit: replicas with no
    /// record naming them, which select nothing (spec: KL-3, KL-12).
    RefuseHead,
    /// Lets a sibling take the snapshot slot first, then reports the loss.
    ///
    /// The bytes that land are the ones this writer was about to write, which is
    /// what makes it a faithful sibling: two Snapshots of one head are the same
    /// checkpoint (spec: CK-11).
    SiblingSnapshot,
    /// Refuses to move anything to the trash, permanently.
    ///
    /// What a Library on credentials that may write but not delete looks like.
    /// The commit itself is untouched — trashing happens after the record exists
    /// — so what this reaches is the settle alone (spec: CP-14, OC-6).
    RefuseTrash,
}

impl<'a> FaultyStore<'a> {
    /// Loses the write of one Keyring replica.
    pub(super) fn swallowing_replica(inner: &'a dyn ObjectStore, replica: u16) -> Self {
        Self {
            inner,
            fault: Fault::SwallowReplica(replica),
        }
    }

    /// Refuses every create aimed at the control-head chain.
    pub(super) fn refusing_the_head(inner: &'a dyn ObjectStore) -> Self {
        Self {
            inner,
            fault: Fault::RefuseHead,
        }
    }

    /// Puts a sibling in the snapshot slot before the create reaches it.
    pub(super) fn losing_the_snapshot_slot(inner: &'a dyn ObjectStore) -> Self {
        Self {
            inner,
            fault: Fault::SiblingSnapshot,
        }
    }

    /// Refuses every move to the trash.
    pub(super) fn refusing_to_trash(inner: &'a dyn ObjectStore) -> Self {
        Self {
            inner,
            fault: Fault::RefuseTrash,
        }
    }

    /// What a provider that will not delete answers with.
    ///
    /// Permanent rather than throttling, so the settle reports it instead of
    /// waiting it out: the retry policy decides from the type alone.
    pub(super) fn trash_refusal() -> Error {
        Error::PermissionDenied {
            detail: "these credentials may write but not delete".to_owned(),
        }
    }
}

/// Whether a name is the replica at `index` of some Keyring generation.
fn is_replica(name: &str, index: u16) -> bool {
    matches!(
        ControlObjectName::parse(name),
        Ok(ControlObjectName::KeyringReplica { replica, .. }) if replica.index() == index
    )
}

/// Whether a name is an ordinary checkpoint (spec: CK-10, FM-12).
fn is_snapshot(name: &str) -> bool {
    matches!(
        ControlObjectName::parse(name),
        Ok(ControlObjectName::IndexSnapshot { .. })
    )
}

#[async_trait]
impl ObjectStore for FaultyStore<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<ObjectRef> {
        if matches!(self.fault, Fault::SwallowReplica(index) if is_replica(name, index)) {
            // Acknowledged and not stored, which is the whole point.
            return Ok(ObjectRef::new(name));
        }
        self.inner.put(name, body).await
    }

    async fn reserve_create(&self, name: &str) -> Result<CommitSlot> {
        self.inner.reserve_create(name).await
    }

    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> Result<ObjectRef> {
        match self.fault {
            Fault::RefuseHead if is_head(slot.name()) => Err(Error::Rejected {
                status: 500,
                detail: "the commit was interrupted before its record was created".to_owned(),
            }),
            Fault::SiblingSnapshot if is_snapshot(slot.name()) => {
                // The sibling wins the slot with the same bytes, and this
                // writer is told what a loser is told.
                self.inner.put_if_absent(slot, body).await?;
                Err(Error::AlreadyExists {
                    object: slot.name().to_owned(),
                })
            }
            _ => self.inner.put_if_absent(slot, body).await,
        }
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
        if matches!(self.fault, Fault::RefuseTrash) {
            return Err(Self::trash_refusal());
        }
        self.inner.trash(object).await
    }

    async fn purge(&self, object: &ObjectRef) -> Result<()> {
        self.inner.purge(object).await
    }
}
