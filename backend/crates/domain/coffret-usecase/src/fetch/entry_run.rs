use coffret_model::EntryPath;
use tracing::info;

use crate::commit::{catch_up, read_committed};
use crate::fetch::entry_fetch::EntryFetch;
use crate::fetch::entry_request::FetchEntryRequest;
use crate::fetch::fetch_error::{FetchError, FetchResult};
use crate::fetch::run::envelope;
use crate::fetch::surfaced::Surfaced;
use crate::fetch::{range_read, select, translate};

/// Makes one Entry available on this device, reading only the part of its
/// Container that holds it.
///
/// [`fetch_folders`](super::fetch_folders) is what makes a folder a copy of its
/// part of the Library, and its unit is the whole Container however many Entries
/// are wanted out of it (spec: PK-16). This is the other thing a reader wants: a
/// page out of a book nobody has fetched yet, now, without waiting for the
/// gigabyte around it. It is the same journey with the same gates and one step
/// done differently — the Container is range-read rather than pulled.
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
///    reported locked, exactly as a folder fetch reports it (spec: KL-7, KL-17).
/// 4. **Range-read the Entry** (spec: FM-2, FM-5, FM-9, PK-16). A Container is
///    self-describing, so its own front says where inside the plaintext stream
///    the Entry sits, and the read is that front plus the chunks covering
///    exactly that extent. The extent comes from the object's entry table
///    rather than from the catalog; what the catalog answers for is the hash
///    the plaintext is then held against (spec: CP-11).
/// 5. **Place** (spec: EP-4, EP-10, EP-11). Temporary file, the Entry's own
///    modification time, the plaintext hash against what the catalog records,
///    rename, then marked present — the same discipline, because it is what
///    makes a fetched file the device's own materialization rather than bytes it
///    happens to have.
///
/// What it does *not* do is claim the Container. A range read cannot check the
/// object's own hash — that is a claim about bytes it deliberately did not ask
/// for — so the integrity gates here are per-chunk authentication for the bytes
/// that arrive and the Entry's plaintext hash against the catalog before the
/// file becomes visible (spec: FM-5, FM-8, CP-11, EP-11). The rest of the
/// Container is as unfetched afterwards as it was before, and completing it is a
/// later run's.
pub async fn fetch_entry(request: FetchEntryRequest<'_>) -> FetchResult<EntryFetch> {
    let FetchEntryRequest {
        store,
        index,
        keys,
        path,
        now,
        policy,
    } = request;

    let caught = catch_up(store, index, keys.control(), &policy.retry).await?;
    let Some(checkpoint) = index.checkpoint().await? else {
        // A Library that has committed nothing holds no current Entry at all
        // (spec: CP-1, FM-13).
        return Err(FetchError::EntryNotCurrent { path });
    };

    // The mappings decide where an Entry's file could go, and the prefix that
    // narrows them here is the Entry Path itself (spec: EP-9).
    let mut translated = translate::targets(index, Some(&path)).await?;
    translated.retain(|target| target.path() == &path);
    let Some(target) = translated.pop() else {
        return Err(match index.entry_at(&path).await? {
            Some(_) => FetchError::UnmappedEntryPath { path },
            None => FetchError::EntryNotCurrent { path },
        });
    };

    let mut selection = select::select(index, vec![target]).await?;
    if let Some(surfaced) = selection.surfaced.pop() {
        finished(&path, "surfaced");
        return Ok(EntryFetch::Surfaced(surfaced));
    }
    let Some(target) = selection.wanted.pop() else {
        finished(&path, "already present");
        return Ok(EntryFetch::AlreadyPresent);
    };

    // One valid replica carries the whole Keyring, so the count is redundancy
    // and never a quorum (spec: KL-6).
    let keyring = read_committed(
        store,
        keys.control(),
        &policy.retry,
        &caught.listing,
        &checkpoint.keyring,
    )
    .await?;
    let container_id = target.location.container_id;
    let Some(envelope) = envelope(&keyring, container_id)? else {
        // Present but locked: the ciphertext stays where it is and the Entry is
        // reported rather than read (spec: KL-7, KL-17, RV-2, RV-7).
        finished(&path, "locked");
        return Ok(EntryFetch::Surfaced(Surfaced::KeyLost {
            path: target.location.entry.path,
            container_id,
        }));
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

    let placement = range_read::read_entry(
        store,
        &policy.retry,
        keys,
        &caught.listing,
        &summary,
        &envelope,
        &target,
    )
    .await?;
    placement.publish(index, now).await?;

    finished(&path, "placed");
    Ok(EntryFetch::Placed)
}

/// Records what one partial fetch came to.
///
/// The Entry Path never reaches a log line, so what is recorded is the verdict
/// and how long the path was — enough to read a run's account of itself without
/// naming what the user has.
fn finished(path: &EntryPath, verdict: &'static str) {
    info!(
        verdict,
        path_len = path.as_str().len(),
        "a partial fetch finished",
    );
}
