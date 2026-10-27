use std::sync::Arc;

use coffret_model::IndexCheckpoint;
use tracing::debug;

use crate::commit::ControlListing;
use crate::fetch::fetch_error::FetchResult;
use crate::fetch::fetch_memory::{FetchMemory, Turn};
use crate::fetch::storage_reads::Origin;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// The listing the run reaches objects through, out of `memory` where it was
/// taken at the head the catalog stands at, and where it came from.
///
/// Where it was not — nothing has been kept since the Library was opened, or
/// the catalog has moved since, by a commit of this device's own or a
/// catch-up that kept nothing — Storage is listed once, and what it says is
/// kept for the next run. Listing is not catching up: the catalog stays where
/// it is, and the listing serves only as the handles the objects it names are
/// reached by (spec: FM-3, FM-12).
///
/// Where another run is listing at that head already — a page opened while its
/// folder's fronts are read ahead — this one waits for that listing rather
/// than taking a second, and meets the refusal it met where it failed. A
/// listing dropped before it landed leaves this run to list for itself.
pub(in crate::fetch) async fn listed(
    store: &dyn ObjectStore,
    index: &dyn Index,
    retry: &RetryPolicy,
    memory: &FetchMemory,
    checkpoint: &IndexCheckpoint,
) -> FetchResult<(Arc<ControlListing>, Origin)> {
    let head = Some(checkpoint.head_generation());
    loop {
        let flight = match memory.listing_at(head) {
            Turn::Kept(listing) => return Ok((listing, Origin::Kept)),
            Turn::Join(waiter) => {
                match waiter.landed().await {
                    Some(landed) => {
                        debug!(
                            listed = landed.is_ok(),
                            "Storage was being listed already, so this run waited for that listing",
                        );
                        return Ok((landed?, Origin::Joined));
                    }
                    None => {
                        debug!("the listing this run waited for was dropped, so it lists Storage itself");
                        continue;
                    }
                }
            }
            Turn::Read(flight) => flight,
        };
        debug!("no listing kept at the head the catalog stands at, so Storage is listed");
        let listing = match ControlListing::read(store, retry).await {
            Ok(listing) => Arc::new(listing),
            Err(error) => {
                flight.land(Err(error.clone()));
                return Err(error.into());
            }
        };
        // Landed before it is kept: keeping it asks the catalog which
        // Containers are current, and a waiter needs the listing, not that.
        // Nor does this run: where the catalog will not answer, the listing in
        // hand still reaches the objects, and only the next run lists again.
        flight.land(Ok(Arc::clone(&listing)));
        memory
            .remember_listing(index, Arc::clone(&listing), head)
            .await;
        return Ok((listing, Origin::Read));
    }
}

#[cfg(test)]
mod tests {
    use coffret_logging::testing::CapturedLogs;
    use coffret_model::{MasterKey, MasterKeyEpoch};
    use tracing::Level;

    use super::listed;
    use crate::catch_up::{catch_up_catalog, CatchUpRequest};
    use crate::commit::{write_first_head, CommitPolicy};
    use crate::device_state::DeviceTime;
    use crate::fetch::storage_reads::Origin;
    use crate::fetch::FetchMemory;
    use crate::freeze::LibraryKeys;
    use crate::in_memory_fs::InMemoryFs;
    use crate::in_memory_store::InMemoryStore;
    use crate::index::Index;
    use crate::refusing_index::RefusingIndex;

    // The listing in hand reaches the objects whether or not it is kept, so a
    // catalog that will not say which Containers are current costs the next run
    // a listing of its own and this one nothing: the run answers with what it
    // listed, and the refusal is logged.
    #[tokio::test]
    async fn a_listing_that_is_not_kept_still_serves_the_run() {
        let keys = LibraryKeys::derive(
            &MasterKey::from_bytes([0x5a; MasterKey::BYTE_LEN]),
            MasterKeyEpoch::FIRST,
        );
        let store = InMemoryStore::new(8);
        let policy = CommitPolicy::default();
        write_first_head(&store, keys.control(), &policy, None)
            .await
            .expect("creating a Library in an empty store must succeed")
            .expect("an empty store holds no head to find instead");
        let index = RefusingIndex::refusing_containers();
        let fs = InMemoryFs::new();
        catch_up_catalog(CatchUpRequest::new(
            &store,
            &index,
            &keys,
            &fs,
            DeviceTime::from_unix_seconds(1_700_000_000),
        ))
        .await
        .unwrap_or_else(|error| panic!("the catch-up must answer: {error}"));
        let checkpoint = index
            .checkpoint()
            .await
            .expect("the catalog says where it stands")
            .expect("the catalog stands at the first head");

        let memory = FetchMemory::default();
        let logs = CapturedLogs::capture();
        let (listing, origin) = listed(&store, &index, &policy.retry, &memory, &checkpoint)
            .await
            .unwrap_or_else(|error| panic!("the run must have its listing: {error}"));

        assert!(matches!(origin, Origin::Read));
        assert_eq!(
            listing.newest_head(),
            Some(checkpoint.head_generation()),
            "the listing the run reaches objects through is the one it took",
        );
        let warned = logs.only(Level::WARN);
        assert!(warned.message().contains("listing is not kept"), "{warned}");

        let (_, again) = listed(&store, &index, &policy.retry, &memory, &checkpoint)
            .await
            .unwrap_or_else(|error| panic!("the next run must have its listing: {error}"));
        assert!(
            matches!(again, Origin::Read),
            "nothing was kept, so the next run lists Storage again",
        );
    }
}
