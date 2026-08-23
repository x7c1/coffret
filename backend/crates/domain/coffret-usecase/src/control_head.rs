use coffret_model::{ControlObjectName, Generation};

use crate::commit_slot::CommitSlot;
use crate::error::Result;
use crate::object_store::ObjectStore;

/// The head of a Library's control-head chain, as far as reserving from it
/// needs it.
///
/// Every authenticated control head — a Journal record, or the Index Snapshot
/// that activated the current epoch — determines exactly one next commit slot
/// and exactly one place its ordinary checkpoint goes (spec: CP-2, CK-10). Both
/// follow from the head's generation alone, so this type is that generation and
/// the two derivations, and it knows nothing about what a commit or a checkpoint
/// contains.
///
/// The derivations are here, above the port, rather than inside an adapter,
/// because they are the reason the exclusion works: a store sees bytes and
/// names, never kinds, so it is at this layer that a Journal record and an
/// epoch activation are made to aim at one name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlHead {
    generation: Generation,
}

impl ControlHead {
    /// The head at `generation`.
    pub const fn at(generation: Generation) -> Self {
        Self { generation }
    }

    /// Where this head sits in the Library's control history.
    pub const fn generation(&self) -> Generation {
        self.generation
    }

    /// The name this head's successor is created under, whatever kind it is
    /// (spec: CP-2, CP-3, FM-12, FM-13).
    pub fn successor_name(&self) -> Result<ControlObjectName> {
        Ok(ControlObjectName::successor_of(self.generation)?)
    }

    /// The name of the ordinary Index Snapshot that checkpoints this head.
    ///
    /// A checkpoint is not a successor: it represents the head itself, so it
    /// takes the head's own generation and a name of its own (spec: CK-10,
    /// FM-12).
    pub fn snapshot_name(&self) -> ControlObjectName {
        ControlObjectName::index_snapshot(self.generation)
    }

    /// Reserves the slot this head's successor is committed into.
    pub async fn reserve_commit_slot(&self, store: &dyn ObjectStore) -> Result<CommitSlot> {
        store
            .reserve_create(&self.successor_name()?.to_string())
            .await
    }

    /// Reserves the slot this head's ordinary Index Snapshot is written into.
    pub async fn reserve_snapshot_slot(&self, store: &dyn ObjectStore) -> Result<CommitSlot> {
        store
            .reserve_create(&self.snapshot_name().to_string())
            .await
    }
}

#[cfg(test)]
mod tests {
    use coffret_model::ControlObjectKind;

    use super::*;
    use crate::byte_stream::ByteStream;
    use crate::error::Error;
    use crate::in_memory_store::InMemoryStore;

    /// Objects small enough that a listing page never matters here.
    const PAGE_SIZE: usize = 8;

    /// The head the cases commit from.
    fn head() -> ControlHead {
        ControlHead::at(Generation::new(4))
    }

    /// The slot a successor of `head` of this kind is committed into.
    ///
    /// The kind is checked against the name rather than spelled into it, which
    /// is the whole point: the derivation takes no kind, and the admission
    /// table (FM-12) is what says the kind belongs at this name.
    async fn successor_slot(
        head: &ControlHead,
        kind: ControlObjectKind,
        store: &InMemoryStore,
    ) -> CommitSlot {
        let name = head
            .successor_name()
            .expect("a head below the last generation has a successor");
        assert!(name.admits(kind), "{name} must admit a {kind:?} successor");

        store
            .reserve_create(&name.to_string())
            .await
            .expect("reserving a commit slot must succeed")
    }

    // CP-2, CP-3, FM-12, FM-13: a head determines one successor slot, and both
    // successor kinds derive it. Naming the two kinds differently is exactly
    // what made this false on a name-keyed store: an ordinary commit and an
    // epoch activation aimed at two keys and both succeeded, so activation
    // fenced nobody.
    #[tokio::test]
    async fn commit_slot_is_kind_independent() {
        let store = InMemoryStore::new(PAGE_SIZE);
        let head = head();

        let journal = successor_slot(&head, ControlObjectKind::Journal, &store).await;
        let activation = successor_slot(&head, ControlObjectKind::ActivationSnapshot, &store).await;

        assert_eq!(journal, activation);
        assert_eq!(journal.name(), "head-5.cfrt");

        let record = b"the Journal record".to_vec();
        let snapshot = b"the activation Index Snapshot".to_vec();
        let (committed, activated) = tokio::join!(
            store.put_if_absent(&journal, ByteStream::from(record.clone())),
            store.put_if_absent(&activation, ByteStream::from(snapshot.clone())),
        );

        let winner = match (&committed, &activated) {
            (Ok(_), Err(Error::AlreadyExists { object })) => {
                assert_eq!(object, "head-5.cfrt");
                record
            }
            (Err(Error::AlreadyExists { object }), Ok(_)) => {
                assert_eq!(object, "head-5.cfrt");
                snapshot
            }
            outcomes => panic!("expected exactly one winner, got {outcomes:?}"),
        };

        // The loser reads the slot back to find out what took it, through the
        // slot rather than by name (spec: CP-4, CP-5).
        let object = store
            .object_at(&journal)
            .expect("a spent slot must name the object it holds");
        let stored = store
            .get(&object, None)
            .await
            .expect("the winner's object must be readable back")
            .into_bytes()
            .await
            .expect("the stream is as long as it claims");

        assert_eq!(stored, winner);
    }

    // CK-10, FM-12: a head's checkpoint is not its successor. It represents the
    // head itself, so it takes the head's own generation and its own name, and
    // spending the commit slot on it would put a checkpoint where the next
    // commit belongs.
    #[tokio::test]
    async fn the_snapshot_slot_is_not_the_commit_slot() {
        let store = InMemoryStore::new(PAGE_SIZE);
        let head = head();

        let commit = head
            .reserve_commit_slot(&store)
            .await
            .expect("reserving a commit slot must succeed");
        let snapshot = head
            .reserve_snapshot_slot(&store)
            .await
            .expect("reserving a snapshot slot must succeed");

        assert_eq!(commit.name(), "head-5.cfrt");
        assert_eq!(snapshot.name(), "idx-4.cfrt");
        assert_ne!(commit, snapshot);
    }

    // CP-2: reserving is not creating, and on a name-keyed store it is
    // idempotent per name — two devices that derive the same successor name
    // reserve the same slot, and the create is still the only thing that
    // settles which of them commits.
    #[tokio::test]
    async fn reserving_the_same_name_twice_yields_the_same_slot() {
        let store = InMemoryStore::new(PAGE_SIZE);
        let head = head();

        let first = head
            .reserve_commit_slot(&store)
            .await
            .expect("reserving a commit slot must succeed");
        let second = head
            .reserve_commit_slot(&store)
            .await
            .expect("reserving a commit slot must succeed");

        assert_eq!(first, second);
        let error = store
            .get(
                &store.object_at(&first).expect("the slot names an object"),
                None,
            )
            .await
            .expect_err("reserving must not create anything");
        assert!(
            matches!(error, Error::NotFound { .. }),
            "expected an unspent slot to hold nothing, got {error:?}"
        );
    }

    // FM-13: the last representable generation has no successor to commit into,
    // and a head that cannot name one reserves nothing rather than reserving
    // something wrong.
    #[tokio::test]
    async fn the_last_generation_has_no_successor_slot() {
        let store = InMemoryStore::new(PAGE_SIZE);
        let head = ControlHead::at(Generation::new(u64::MAX));

        let result = head.reserve_commit_slot(&store).await;
        assert!(
            matches!(
                result,
                Err(Error::Model(coffret_model::Error::GenerationOutOfRange))
            ),
            "expected the last generation to name no successor, got {result:?}"
        );
    }
}
