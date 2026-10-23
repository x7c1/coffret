use std::collections::BTreeSet;
use std::sync::Arc;

use coffret_model::{ContainerId, Generation, Redacted};
use tracing::warn;

use super::{FetchMemory, Flight, Turn};
use crate::commit::{CommitError, ControlListing};
use crate::index::Index;

/// What one read of the listing came to.
pub(crate) type ListingRead = Result<Arc<ControlListing>, CommitError>;

/// One listing, and the head the catalog stood at when it was taken.
#[derive(Debug)]
pub(super) struct Listed {
    at: Option<Generation>,
    listing: Arc<ControlListing>,
}

impl FetchMemory {
    /// The listing taken while the catalog stood at `head`, where one was; a
    /// wait on the one being taken there, where that is under way; and the
    /// read to make, where neither is.
    pub(crate) fn listing_at(
        &self,
        head: Option<Generation>,
    ) -> Turn<'_, Arc<ControlListing>, Option<Generation>, ListingRead> {
        let mut kept = self.locked();
        if let Some(listed) = kept.listing.as_ref().filter(|listed| listed.at == head) {
            return Turn::Kept(Arc::clone(&listed.listing));
        }
        if let Some(waiter) = kept.listing_reads.join(&head) {
            return Turn::Join(waiter);
        }
        Turn::Read(Flight::start(
            self,
            &mut kept,
            |kept| &mut kept.listing_reads,
            head,
        ))
    }

    /// Keeps a listing taken while the catalog stood at `at`, and forgets the
    /// front of every Container `index` no longer holds a current Entry in.
    ///
    /// Infallible, because what it keeps is a cache: every caller already
    /// holds the answer the listing served — a head compared, a catalog caught
    /// up, the handles an Entry fetch reaches objects by — and a catalog that
    /// would not say which Containers are current takes nothing from that
    /// answer. So the refusal is logged and nothing is kept, not even the
    /// listing, whose fronts could then not be pruned with it; the next read
    /// lists Storage again.
    pub(crate) async fn remember_listing(
        &self,
        index: &dyn Index,
        listing: Arc<ControlListing>,
        at: Option<Generation>,
    ) {
        let current: BTreeSet<ContainerId> = match index.containers_under(None).await {
            Ok(containers) => containers
                .into_iter()
                .map(|container| container.id)
                .collect(),
            Err(error) => {
                warn!(
                    reason = %error.redacted(),
                    "the catalog would not say which Containers are current, so the listing \
                     is not kept; the next read lists Storage again",
                );
                return;
            }
        };
        self.keep_listing(listing, at, &current);
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
