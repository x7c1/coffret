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
        flight.land(Ok(Arc::clone(&listing)));
        memory
            .remember_listing(index, Arc::clone(&listing), head)
            .await?;
        return Ok((listing, Origin::Read));
    }
}
