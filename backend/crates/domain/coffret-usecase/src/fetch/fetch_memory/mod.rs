use std::collections::BTreeSet;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use coffret_model::{ContainerId, Generation, KeyTable, KeyringCommitment};

use crate::commit::{ControlListing, DegradedKeyring};
use crate::index::Index;
use crate::index_error::IndexResult;

mod fronts;
use fronts::Fronts;

/// What a process that fetches many Entries keeps between them, so that the
/// second one does not ask Storage again for what the first already learned.
///
/// Every Entry fetch needs three things before it reads a parcel: the control
/// objects the catalog's head was read from (the handles a Keyring replica and a
/// Container this device never uploaded are reached by, spec: FM-3, FM-12), the
/// committed key table (spec: KL-1, KL-6), and the front of the Container it
/// reads (spec: FM-2, PK-16). None of the three changes between two fetches that
/// stand on one catalog, and a process serving a reader asks for many Entries
/// over one catalog. So it keeps them, each for exactly as long as it stays
/// true:
///
/// - **The listing** is kept with the head the catalog stood at when it was
///   taken. A catalog standing anywhere else — a catch-up advanced it, or a
///   commit of this device's own did — is one the listing may not cover, and
///   the next fetch lists again.
/// - **The key table** is kept with the commitment it was read under, and only
///   handed back for that commitment (spec: KL-1). A catalog whose checkpoint
///   names another Keyring has its fetch read that one.
/// - **A Container's front** is kept by Container ID, within a bound on how many
///   and how many bytes, and forgotten once the Container leaves the current
///   set. A Container object never changes once committed (spec: FM-2, CP-11),
///   so the bytes are the object's own for as long as it is current. What is
///   kept is the ciphertext as it arrived; it is opened under the Container's
///   key each time it is used, exactly as a front read from Storage is, and a
///   front served from here shows the provider less than a read would, never
///   more (spec: PK-20).
///
/// The key table holds envelopes wrapped under the Library's wrap key, and the
/// fronts hold sealed meta sections: neither opens anything without the keys.
/// It is kept beside the keys all the same, and goes when they do — a device
/// that has locked holds nothing a reader could use (spec: DK-2, DK-4). That is
/// why it is a value the open Library carries rather than one the process
/// does: the lock drops it with the keys, and the unlock starts from nothing.
///
/// Nothing here is a reason to read anything. A fetch handed this answers from
/// the catalog as it stands and catches nothing up by itself; the catalog moves
/// when its owner catches it up, and records what it read here on the way
/// (see [`CatchUpRequest::remembering`](crate::catch_up::CatchUpRequest::remembering)).
#[derive(Debug, Default)]
pub struct FetchMemory {
    kept: Mutex<Kept>,
}

#[derive(Debug, Default)]
struct Kept {
    listing: Option<Listed>,
    keyring: Option<Opened>,
    fronts: Fronts,
}

/// One listing, and the head the catalog stood at when it was taken.
#[derive(Debug)]
struct Listed {
    at: Option<Generation>,
    listing: Arc<ControlListing>,
}

/// One committed key table, and the commitment it was read under.
#[derive(Debug)]
struct Opened {
    commitment: KeyringCommitment,
    key_table: Arc<KeyTable>,
    degraded: Option<DegradedKeyring>,
}

impl FetchMemory {
    /// The listing taken while the catalog stood at `head`, where one was.
    pub(crate) fn listing_at(&self, head: Option<Generation>) -> Option<Arc<ControlListing>> {
        self.locked()
            .listing
            .as_ref()
            .filter(|listed| listed.at == head)
            .map(|listed| Arc::clone(&listed.listing))
    }

    /// Keeps a listing taken while the catalog stood at `at`, and forgets the
    /// front of every Container `index` no longer holds a current Entry in.
    pub(crate) async fn remember_listing(
        &self,
        index: &dyn Index,
        listing: Arc<ControlListing>,
        at: Option<Generation>,
    ) -> IndexResult<()> {
        let current: BTreeSet<ContainerId> = index
            .containers_under(None)
            .await?
            .into_iter()
            .map(|container| container.id)
            .collect();
        self.keep_listing(listing, at, &current);
        Ok(())
    }

    fn keep_listing(
        &self,
        listing: Arc<ControlListing>,
        at: Option<Generation>,
        current: &BTreeSet<ContainerId>,
    ) {
        let mut kept = self.locked();
        kept.listing = Some(Listed { at, listing });
        kept.fronts.retain(current);
    }

    /// The key table read under `commitment`, and what that read found of the
    /// set, where it was read.
    pub(crate) fn key_table_for(
        &self,
        commitment: &KeyringCommitment,
    ) -> Option<(Arc<KeyTable>, Option<DegradedKeyring>)> {
        self.locked()
            .keyring
            .as_ref()
            .filter(|opened| &opened.commitment == commitment)
            .map(|opened| (Arc::clone(&opened.key_table), opened.degraded))
    }

    /// Keeps the key table read under `commitment`, in place of any other.
    pub(crate) fn remember_key_table(
        &self,
        commitment: KeyringCommitment,
        key_table: Arc<KeyTable>,
        degraded: Option<DegradedKeyring>,
    ) {
        self.locked().keyring = Some(Opened {
            commitment,
            key_table,
            degraded,
        });
    }

    /// The front of one Container as it arrived, where it is kept.
    pub(crate) fn front(&self, container_id: ContainerId) -> Option<Arc<[u8]>> {
        self.locked().fronts.get(container_id)
    }

    /// Keeps the front of one Container, making room by forgetting the oldest.
    pub(crate) fn remember_front(&self, container_id: ContainerId, front: Arc<[u8]>) {
        self.locked().fronts.keep(container_id, front);
    }

    fn locked(&self) -> MutexGuard<'_, Kept> {
        // Every write under this lock replaces one field or edits the queue in
        // one step, so what a panic left behind is still a memory — at worst
        // one that forgot something and reads it again.
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use coffret_model::ContainerId;

    use super::FetchMemory;

    fn container(byte: u8) -> ContainerId {
        ContainerId::from_bytes([byte; ContainerId::BYTE_LEN])
    }

    fn front(len: usize) -> Arc<[u8]> {
        vec![0u8; len].into()
    }

    // A Container that leaves the current set takes its front with it.
    #[test]
    fn a_container_no_longer_current_is_forgotten() {
        let memory = FetchMemory::default();
        memory.remember_front(container(1), front(32));
        memory.remember_front(container(2), front(32));

        memory.keep_listing(Arc::default(), None, &BTreeSet::from([container(2)]));

        assert!(memory.front(container(1)).is_none());
        assert!(memory.front(container(2)).is_some());
    }
}
