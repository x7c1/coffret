use coffret_model::EntryPath;
use tracing::info;

use crate::commit::{catch_up, read_committed};
use crate::fetch::entry_fetch::{EntryFetch, EntryFetchOutcome};
use crate::fetch::entry_request::FetchEntryRequest;
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::parcel_read::let_go::let_go_departed;
use crate::fetch::parcel_read::ParcelRead;
use crate::fetch::reading::Reading;
use crate::fetch::run::envelope;
use crate::fetch::surfaced::Surfaced;
use crate::fetch::{select, translate};
use crate::progress::{Phase, Step};

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
///    nothing from a catalog that has not been brought to the head.
/// 2. **Translate the Entry Path into a local path** (spec: EP-9), and decide
///    whether this device may write there (spec: EP-10, EP-11). A path the
///    mappings do not cover is not a path a fetch can place a file at, and a
///    path whose local state this device cannot vouch for is a finding rather
///    than something to overwrite.
/// 3. **Open the committed Keyring** (spec: KL-1, KL-3, KL-6) and take the
///    envelope it maps this Entry's Container to. One it records as key-lost is
///    reported unreadable, exactly as a folder fetch reports it (spec: KL-7, KL-17).
/// 4. **Read the Entry's parcels** (spec: FM-2, FM-5, FM-9, PK-16, PK-19,
///    PK-21). A Container is self-describing, so its own front says where
///    inside the plaintext stream the Entry sits and so which parcels it
///    overlaps; the read is that front plus each of those parcels this device
///    does not already hold, whole, and the parcels are kept. The extent comes
///    from the object's entry table rather than from the catalog; what the
///    catalog answers for is the hash the plaintext is then held against
///    (spec: CP-11). The other Entries those parcels wholly cover come with it
///    and are placed too, where this device would place them anyway.
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
    } = request;

    // The same phases a folder fetch says, in the same order, so a caller
    // renders one Entry exactly as it renders a folder: the catch-up is the
    // same catch-up, and on a device that has just joined it is still the
    // longest part of the run.
    progress.step(Step::begun(Phase::CatchingUp));
    let caught = catch_up(store, index, keys.control(), &policy.retry).await?;
    // A held parcel whose Container the catch-up took out of the current set
    // serves nothing any more (spec: PK-21).
    let_go_departed(index, &parcels).await?;
    let Some(checkpoint) = index.checkpoint().await? else {
        // A Library that has committed nothing holds no current Entry at all
        // (spec: CP-1, FM-13).
        return Err(FetchError::EntryNotCurrent { path });
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

    // One valid replica carries the whole Keyring, so the count is redundancy
    // and never a quorum (spec: KL-6).
    // Reported here and not held: a fetch writes nothing, so nothing later in
    // this run examines the set the key table came from. The finding goes on the
    // outcome as well, for whoever asked for the Entry (spec: KL-15).
    let (key_table, degraded) = read_committed(
        store,
        keys.control(),
        &policy.retry,
        &caught.listing,
        checkpoint.keyring(),
    )
    .await?
    .reported();
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
            degraded,
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
        listing: &caught.listing,
    };
    let read = ParcelRead {
        reading: &reading,
        kept: &parcels,
        index,
        now,
        cancellation,
        publication,
        degraded,
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
        degraded,
        alongside: stroke.alongside,
        unheld: stroke.unheld,
    })
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
