use std::ops::Range;
use std::sync::Mutex;

use async_trait::async_trait;
use coffret_model::{ContainerKind, ControlObjectName, Mtime, ObjectRef};

use super::fixtures::plant;
use crate::byte_stream::ByteStream;
use crate::commit_slot::CommitSlot;
use crate::error::{Error, Result};
use crate::index::Index;
use crate::object_page::ObjectPage;
use crate::object_store::ObjectStore;
use crate::page_token::PageToken;
use crate::sync::LibraryKeys;
use crate::uploaded_object::UploadedObject;

/// What becomes of a Journal create this device sends into a free slot.
#[derive(Clone, Copy)]
pub(super) enum Answer {
    /// Nothing reaches Storage, and no answer comes back.
    Lost,
    /// The record lands, and the answer is lost on its way back.
    LostAfterLanding,
}

/// A Storage whose answer to a Journal create into a free slot never arrives.
///
/// A create into a slot that is already taken is refused as it would be, which
/// is how a batch that loses the race goes on to rebase (spec: CP-4). Where a
/// rival is set, it commits from its own catalog the moment this device's first
/// create is about to reach Storage, into the slot that create aims at.
pub(super) struct Unanswered<'a> {
    inner: &'a dyn ObjectStore,
    answer: Answer,
    rival: Mutex<Option<(&'a dyn Index, &'static str, &'a LibraryKeys)>>,
}

impl<'a> Unanswered<'a> {
    /// Answers as `answer` does, around the store the suite was handed.
    pub(super) fn around(
        inner: &'a dyn ObjectStore,
        answer: Answer,
        rival: Option<(&'a dyn Index, &'static str, &'a LibraryKeys)>,
    ) -> Self {
        Self {
            inner,
            answer,
            rival: Mutex::new(rival),
        }
    }

    /// Whether something is already at the slot.
    async fn taken(&self, slot: &CommitSlot) -> Result<bool> {
        let object = self.inner.object_at(slot)?;
        match self.inner.get(&object, None).await {
            Ok(_) => Ok(true),
            Err(Error::NotFound { .. }) => Ok(false),
            Err(error) => Err(error),
        }
    }
}

fn is_head(name: &str) -> bool {
    matches!(
        ControlObjectName::parse(name),
        Ok(ControlObjectName::Head { .. })
    )
}

#[async_trait]
impl ObjectStore for Unanswered<'_> {
    async fn put(&self, name: &str, body: ByteStream) -> Result<UploadedObject> {
        self.inner.put(name, body).await
    }

    async fn reserve_create(&self, name: &str) -> Result<CommitSlot> {
        self.inner.reserve_create(name).await
    }

    async fn put_if_absent(&self, slot: &CommitSlot, body: ByteStream) -> Result<ObjectRef> {
        if !is_head(slot.name()) {
            return self.inner.put_if_absent(slot, body).await;
        }
        let rival = self
            .rival
            .lock()
            .expect("the rival is only ever taken")
            .take();
        if let Some((index, path, keys)) = rival {
            plant(
                self.inner,
                index,
                keys,
                ContainerKind::OneFile,
                path,
                b"another writer's file",
                Mtime::from_unix_seconds(1_700_000_000),
                false,
            )
            .await;
        }
        if self.taken(slot).await? {
            return self.inner.put_if_absent(slot, body).await;
        }
        if matches!(self.answer, Answer::LostAfterLanding) {
            self.inner.put_if_absent(slot, body).await?;
        }
        // A real lost answer reaches the caller as a timeout, which the retry
        // policy would resend. The case needs the run to stop at this one create
        // with its row marked, so the fake answers with a permanent failure,
        // which the retry policy reports at once.
        Err(Error::PermissionDenied {
            detail: "the answer to the create never arrived".to_owned(),
            source: None,
        })
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
