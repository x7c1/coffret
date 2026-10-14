use std::sync::Arc;

use coffret_model::{EntryPath, IndexCheckpoint, KeyTable, KeyringCommitment};
use tracing::{debug, info};

use crate::commit::{catch_up, read_committed, ControlListing, ShortKeyring};
use crate::fetch::entry_fetch::{EntryFetch, EntryFetchOutcome};
use crate::fetch::entry_request::FetchEntryRequest;
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::fetch_memory::FetchMemory;
use crate::fetch::parcel_read::let_go::let_go_departed;
use crate::fetch::parcel_read::ParcelRead;
use crate::fetch::reading::Reading;
use crate::fetch::run::envelope;
use crate::fetch::surfaced::Surfaced;
use crate::fetch::{select, translate};
use crate::index::Index;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;
use crate::progress::{Phase, Step};
use crate::retry::RetryPolicy;

/// Makes one Entry available on this device, reading only the parcels of its
/// Container that hold it.
///
/// [`fetch_folders`](super::fetch_folders) is what makes a folder a copy of its
/// part of the Library, and it reads whole Containers — every parcel at once
/// (spec: PK-16). This is the other thing a reader wants: a page out of a book
/// nobody has fetched yet, now, without waiting for the gigabyte around it. It
/// is the same journey with the same gates and one step done differently — the
/// Container is read by its parcels rather than pulled whole.
///
/// The steps, and the rule each answers to:
///
/// 1. **Catch up** (spec: CK-9, RV-1). The same first step, for the same reason:
///    an answer about what the Library currently holds at a path is worth
///    nothing from a catalog that has not been brought to the head. A run
///    handed a [`FetchMemory`] skips it and answers from the catalog as it
///    stands: its owner catches that catalog up, and keeps the listing the
///    catch-up walked where this run finds it. Where the memory keeps no
///    listing of the head the catalog stands at, Storage is listed once —
///    listed, not replayed — and the listing kept.
/// 2. **Translate the Entry Path into a local path** (spec: EP-9), and decide
///    whether this device may write there (spec: EP-10, EP-11). A path the
///    mappings do not cover is not a path a fetch can place a file at, and a
///    path whose local state this device cannot vouch for is a finding rather
///    than something to overwrite.
/// 3. **Open the committed Keyring** (spec: KL-1, KL-3, KL-6) and take the
///    envelope it maps this Entry's Container to — out of the memory where it
///    keeps the table read under the commitment the catalog's checkpoint
///    names. One it records as key-lost is reported unreadable, exactly as a
///    folder fetch reports it (spec: KL-7, KL-17).
/// 4. **Read the Entry's parcels** (spec: FM-2, FM-5, FM-9, PK-16, PK-19,
///    PK-21). A Container is self-describing, so its own front says where
///    inside the plaintext stream the Entry sits and so which parcels it
///    overlaps; the read is that front — unless the memory keeps it — plus each
///    of those parcels this device does not already hold, whole, and the
///    parcels are kept. The extent comes from the object's entry table rather
///    than from the catalog; what the catalog answers for is the hash the
///    plaintext is then held against (spec: CP-11). The other Entries those
///    parcels wholly cover come with it and are placed too, where this device
///    would place them anyway.
/// 5. **Place** (spec: EP-4, EP-10, EP-11, EP-13). Scratch, the Entry's
///    own modification time, the plaintext hash against what the catalog
///    records, rename, then marked present — the same discipline, because it is
///    what makes a fetched file the device's own materialization rather than
///    bytes it happens to have. As the mapped root is opened it is held against
///    the identity its mapping recorded, and a root that will not vouch for
///    itself *fails* this fetch: there is one Entry and one mapping here, so a
///    refusal leaves nothing to go on with — where
///    [`fetch_folders`](super::fetch_folders) reports the same refusal once per
///    mapping and places into the device's others as usual.
///
/// What it does *not* do is claim the Container. A read of parcels cannot check
/// the object's own hash — that is a claim about bytes it did not ask for — so
/// the integrity gates here are per-chunk authentication for the bytes that
/// arrive and each placed Entry's plaintext hash against the catalog before the
/// file becomes visible (spec: FM-5, FM-8, CP-11, EP-11, PK-22). The parcels
/// it did not read are as unread afterwards as they were before.
///
/// A caller that may stop wanting the Entry hands a
/// [`Cancellation`](super::Cancellation) over, asked before each parcel is
/// requested from Storage and never inside one; a cancelled run answers
/// [`EntryFetch::Cancelled`] with the parcels it read still held (spec: PK-21).
///
/// A caller with a reader waiting hands a [`Publication`](super::Publication)
/// over, told the moment the Entry is published (spec: PK-16). The run itself
/// returns only once the whole stroke is done: every parcel it asked for read
/// to its end and kept, the other Entries they cover placed, and the parcels
/// nothing waits for any more let go (spec: PK-21).
pub async fn fetch_entry(request: FetchEntryRequest<'_>) -> FetchResult<EntryFetchOutcome> {
    let FetchEntryRequest {
        store,
        index,
        keys,
        destinations,
        parcels,
        path,
        now,
        progress,
        policy,
        cancellation,
        publication,
        memory,
    } = request;

    let catalog = match memory {
        None => {
            // The same phases a folder fetch says, in the same order, so a
            // caller renders one Entry exactly as it renders a folder: the
            // catch-up is the same catch-up, and on a device that has just
            // joined it is still the longest part of the run.
            progress.step(Step::begun(Phase::CatchingUp));
            let caught = catch_up(store, index, keys.control(), &policy.retry).await?;
            Catalog::CaughtUp(Arc::new(caught.listing))
        }
        // The catalog as it stands: whoever keeps the memory keeps the catalog
        // current.
        Some(memory) => Catalog::AsItStands(memory),
    };
    // A held parcel whose Container is no longer in the current set serves
    // nothing any more, whether this run's catch-up or anything before it took
    // it out (spec: PK-21). Asked of the catalog alone, so it costs Storage
    // nothing.
    let_go_departed(index, &parcels).await?;
    let Some(checkpoint) = index.checkpoint().await? else {
        // A Library that has committed nothing holds no current Entry at all
        // (spec: CP-1, FM-13).
        return Err(FetchError::EntryNotCurrent { path });
    };
    let listing = match catalog {
        Catalog::CaughtUp(listing) => listing,
        Catalog::AsItStands(memory) => {
            listed(store, index, &policy.retry, memory, &checkpoint).await?
        }
    };

    // The mappings decide where an Entry's file could go, and the prefix that
    // narrows them here is the Entry Path itself (spec: EP-9).
    progress.step(Step::begun(Phase::Scanning));
    let target = translate::target_of(index, &path).await?;

    let mut selection = select::select(index, destinations, vec![target]).await?;
    if let Some(surfaced) = selection.surfaced.pop() {
        finished(&path, "surfaced");
        return Ok(EntryFetchOutcome::of(EntryFetch::Surfaced(surfaced)));
    }
    let Some(target) = selection.wanted.pop() else {
        finished(&path, "already present");
        return Ok(EntryFetchOutcome::of(EntryFetch::AlreadyPresent));
    };

    let (key_table, short) = key_table(
        store,
        keys,
        &policy.retry,
        &listing,
        checkpoint.keyring(),
        memory,
    )
    .await?;
    let container_id = target.location.container_id;
    // One Container, counted as a folder fetch counts its Containers: said
    // before the parcel read starts, so a caller has a line up while the read
    // travels, and again once the Entry is placed.
    progress.step(Step::new(Phase::Fetching, 0, 1));
    let Some(envelope) = envelope(&key_table, container_id)? else {
        // Present but unreadable: the ciphertext stays where it is and the Entry is
        // reported rather than read (spec: KL-7, KL-17, RV-2, RV-7). A Container
        // nothing can open is one fewer to wait for, as a folder fetch counts it.
        progress.step(Step::new(Phase::Fetching, 1, 1));
        finished(&path, "key lost");
        return Ok(EntryFetchOutcome {
            short,
            ..EntryFetchOutcome::of(EntryFetch::Surfaced(Surfaced::KeyLost {
                path: target.location.entry.path,
                container_id,
            }))
        });
    };

    // Which Containers are current is what the Journal says rather than what a
    // listing happens to hold (spec: CP-1, OC-1), and the walk under this Entry's
    // own path reaches the one holding it.
    let summary = index
        .containers_under(Some(&path))
        .await?
        .into_iter()
        .find(|container| container.id == container_id)
        .ok_or(FetchError::ContainerUnreachable { container_id })?;

    let reading = Reading {
        store,
        retry: &policy.retry,
        keys,
        destinations,
        listing: &listing,
    };
    let read = ParcelRead {
        reading: &reading,
        kept: &parcels,
        index,
        now,
        cancellation,
        publication,
        short,
        memory,
    };
    let stroke = read.read_entry(&summary, &envelope, target).await?;
    progress.step(Step::new(Phase::Fetching, 1, 1));

    let fetch = if stroke.placed {
        finished(&path, "placed");
        EntryFetch::Placed
    } else {
        finished(&path, "cancelled");
        EntryFetch::Cancelled
    };
    Ok(EntryFetchOutcome {
        fetch,
        short,
        alongside: stroke.alongside,
        unheld: stroke.unheld,
    })
}

/// Where the catalog a run answers from came from.
enum Catalog<'a> {
    /// The run caught it up itself, and this is the listing it walked.
    CaughtUp(Arc<ControlListing>),
    /// It stands where its owner left it, who keeps what fetches learn here.
    AsItStands(&'a FetchMemory),
}

/// The listing the run reaches objects through, out of `memory` where it was
/// taken at the head the catalog stands at.
///
/// Where it was not — nothing has been kept since the Library was opened, or
/// the catalog has moved since, by a commit of this device's own or a
/// catch-up that kept nothing — Storage is listed once, and what it says is
/// kept for the next run. Listing is not catching up: the catalog stays where
/// it is, and the listing serves only as the handles the objects it names are
/// reached by (spec: FM-3, FM-12).
pub(super) async fn listed(
    store: &dyn ObjectStore,
    index: &dyn Index,
    retry: &RetryPolicy,
    memory: &FetchMemory,
    checkpoint: &IndexCheckpoint,
) -> FetchResult<Arc<ControlListing>> {
    let head = Some(checkpoint.head_generation());
    if let Some(listing) = memory.listing_at(head) {
        return Ok(listing);
    }
    debug!("no listing kept at the head the catalog stands at, so Storage is listed");
    let listing = Arc::new(ControlListing::read(store, retry).await?);
    memory
        .remember_listing(index, Arc::clone(&listing), head)
        .await?;
    Ok(listing)
}

/// The committed key table the checkpoint names, and what reading it found of
/// the set.
///
/// One valid replica carries the whole Keyring, so the count is redundancy and
/// never a quorum (spec: KL-6). The finding is reported here and not held: a
/// fetch writes nothing, so nothing later in this run examines the set the key
/// table came from. It goes on the outcome as well, for whoever asked for the
/// Entry (spec: KL-15).
///
/// Out of `memory`, where it keeps the table read under this very commitment:
/// a commitment names its set by digest, so a table read under it is the table
/// every replica of it carries (spec: KL-1, KL-6). The finding that read made
/// goes on this outcome too — the set is as short as it was — and is not said
/// again in the log, where it has been said once already. It is reported only
/// until the set may have been repaired: a repair keeps the commitment
/// (spec: KL-13, KL-14), so the memory forgets a table kept with that finding
/// at the next catch-up and any table once a write run of this process is over,
/// and does not keep a table whose read began before either of those forgets;
/// the fetch after that reads the set again and reports it short only if that
/// read finds it so (spec: KL-15).
pub(super) async fn key_table(
    store: &dyn ObjectStore,
    keys: &LibraryKeys,
    retry: &RetryPolicy,
    listing: &ControlListing,
    commitment: &KeyringCommitment,
    memory: Option<&FetchMemory>,
) -> FetchResult<(Arc<KeyTable>, Option<ShortKeyring>)> {
    if let Some(kept) = memory.and_then(|memory| memory.key_table_for(commitment)) {
        return Ok(kept);
    }
    // Taken before the replicas are read, so that a write run of this process
    // repairing the set while this read is under way leaves what it found
    // unkept rather than put back after the repair forgot it (spec: KL-15).
    let read_under = memory.map(FetchMemory::key_table_generation);
    let (key_table, short) = read_committed(store, keys.control(), retry, listing, commitment)
        .await?
        .reported();
    let key_table = Arc::new(key_table);
    if let (Some(memory), Some(read_under)) = (memory, read_under) {
        memory.remember_key_table(
            read_under,
            commitment.clone(),
            Arc::clone(&key_table),
            short,
        );
    }
    Ok((key_table, short))
}

/// Records what one partial fetch came to.
///
/// The Entry Path never reaches a diagnostic event, so what is recorded is
/// the verdict and how long the path was — enough to read a run's account of
/// itself without naming what the user has.
fn finished(path: &EntryPath, verdict: &'static str) {
    info!(
        verdict,
        path_len = path.as_str().len(),
        "a partial fetch finished",
    );
}
