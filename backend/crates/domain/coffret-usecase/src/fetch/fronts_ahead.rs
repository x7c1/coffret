use std::collections::BTreeMap;

use coffret_format::unwrap_container_key;
use coffret_model::{ContainerId, ContainerSummary, EntryPath, KeyTable, Redacted};
use tracing::{debug, info, warn};

use crate::commit::ControlListing;
use crate::fetch::entry_run::{key_table, listed};
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::fetch_memory::FetchMemory;
use crate::fetch::fronts_request::FrontsRequest;
use crate::fetch::parcel_read::front;
use crate::fetch::run::envelope;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// Reads the front of each of `containers` that `memory` does not keep, and
/// keeps it there, so that the first Entry fetched out of one costs its parcels
/// alone (spec: FM-2, PK-16).
///
/// What a process does when somebody lists a folder: the first page of a book
/// not yet read otherwise costs three round trips — the header, the meta
/// section, and the parcel — and the first two are the same whichever Entry is
/// wanted. The front names no Entry, so what the provider learns from these
/// reads is that the Containers were opened, which the fill a page opens would
/// show it anyway (spec: PK-16, PK-20).
///
/// `containers` are the ones a caller chose, in the order it wants them read;
/// `folder` is where they hold Entries, which narrows the catalog walk that
/// finds their summaries. A Container kept already costs nothing, and where
/// every one is, nothing at all is asked of Storage or of the catalog's
/// checkpoint. Otherwise the listing and the committed key table come out of
/// the memory as an Entry fetch's would — read and kept where they are not —
/// and each front is opened under its Container's key before it is kept, as
/// one read by a fetch is.
///
/// A Container whose front cannot be read or will not open is said and passed
/// over: these reads are ahead of anybody asking, so the fetch that does ask
/// meets the same refusal and is who answers for it. What fails the whole is
/// what every Container would meet — the catalog, the listing, the key table.
///
/// Answers with how many fronts were read.
pub async fn read_fronts(
    request: FrontsRequest<'_>,
    folder: Option<&EntryPath>,
    containers: &[ContainerId],
) -> FetchResult<usize> {
    let FrontsRequest {
        store,
        index,
        keys,
        memory,
        policy,
    } = request;
    let wanted: Vec<ContainerId> = containers
        .iter()
        .copied()
        .filter(|container_id| memory.front(*container_id).is_none())
        .collect();
    if wanted.is_empty() {
        debug!(
            containers = containers.len(),
            "every front a listing would read is kept, so none is read",
        );
        return Ok(0);
    }
    let Some(checkpoint) = index.checkpoint().await? else {
        // A Library that has committed nothing has no Container to open.
        return Ok(0);
    };
    let listing = listed(store, index, &policy.retry, memory, &checkpoint).await?;
    let (key_table, _) = key_table(
        store,
        keys,
        &policy.retry,
        &listing,
        checkpoint.keyring(),
        Some(memory),
    )
    .await?;
    let summaries: BTreeMap<ContainerId, ContainerSummary> = index
        .containers_under(folder)
        .await?
        .into_iter()
        .map(|summary| (summary.id, summary))
        .collect();

    let ahead = Ahead {
        store,
        retry: &policy.retry,
        keys,
        memory,
        listing: &listing,
        key_table: &key_table,
    };
    let mut read = 0;
    for container_id in wanted {
        let Some(summary) = summaries.get(&container_id) else {
            // Gone from the current set since the caller chose it: nothing
            // fetches out of it any more.
            continue;
        };
        match ahead.front_of(summary).await {
            Ok(true) => read += 1,
            Ok(false) => {}
            Err(error) => warn!(
                container = %container_id,
                error = %error.redacted(),
                "a Container's front read ahead of a fetch was refused",
            ),
        }
    }
    info!(
        read,
        "read the fronts of a listed folder's Containers ahead of a fetch",
    );
    Ok(read)
}

/// What every front read ahead for one listing shares.
struct Ahead<'a> {
    store: &'a dyn ObjectStore,
    retry: &'a RetryPolicy,
    keys: &'a LibraryKeys,
    memory: &'a FetchMemory,
    listing: &'a ControlListing,
    key_table: &'a KeyTable,
}

impl Ahead<'_> {
    /// Reads and keeps the front of the Container `summary` describes, and
    /// says whether it did: a Container the Keyring records as key-lost has a
    /// front nothing can open, and is left for the fetch that reports it
    /// (spec: KL-7, KL-17).
    async fn front_of(&self, summary: &ContainerSummary) -> FetchResult<bool> {
        let container_id = summary.id;
        let Some(envelope) = envelope(self.key_table, container_id)? else {
            return Ok(false);
        };
        let object = summary
            .object_ref
            .as_ref()
            .or_else(|| self.listing.container(container_id))
            .ok_or(FetchError::ContainerUnreachable { container_id })?;
        let key = unwrap_container_key(self.keys.container_wrap(), &container_id, &envelope)?;
        front::outline(
            self.store,
            self.retry,
            Some(self.memory),
            container_id,
            object,
            summary.ciphertext_len.get(),
            &key,
        )
        .await?;
        Ok(true)
    }
}
