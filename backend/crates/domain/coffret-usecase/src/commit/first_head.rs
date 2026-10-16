use coffret_model::{Generation, JournalRecord, KeyTable};
use tracing::{debug, info};

use crate::commit::commit_error::CommitResult;
use crate::commit::commit_policy::CommitPolicy;
use crate::commit::committing::Committing;
use crate::commit::control_keys::ControlKeys;
use crate::commit::control_listing::ControlListing;
use crate::commit::journal::{self, Attempted};
use crate::commit::keyring;
use crate::commit::prepared_batch::PreparedBatch;
use crate::object_store::ObjectStore;
use crate::progress::UNWATCHED;

/// Writes a Library's first head, where it has none (spec: CP-2, FM-13).
///
/// What creating a Library leaves on Storage: `head-0`, a Journal record that
/// adds nothing and removes nothing, selecting Keyring generation 0, which maps
/// nothing. What it is for is the slot it carries. Every commit consumes the
/// slot of the head it starts from, and that is what lets exactly one of the
/// writers starting from one head succeed it (spec: CP-3) — but a Library with
/// no head has nothing to carry a slot, so each writer of generation 0 would
/// reserve its own, and on a Storage that mints identifiers both creates would
/// succeed. Writing the first head when the Library is created, before any
/// other device can know of it, means no commit ever starts from no head, and
/// that race never starts.
///
/// Joining a Library writes nothing: the Library it joins already has a head.
///
/// Returns the record written, or `None` where the Library already had a head
/// — one listed before anything was written, or one another writer created
/// into `head-0` first on a Storage that keys objects by name. Either way the
/// Library has a first head when this returns.
///
/// # Errors
///
/// Whatever reading the listing or writing the Keyring generation and the
/// record met; nothing about a Library that already has a head is one.
pub async fn write_first_head(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    policy: &CommitPolicy,
) -> CommitResult<Option<JournalRecord>> {
    let listing = ControlListing::read(store, &policy.retry).await?;
    if let Some(head) = listing.newest_head() {
        debug!(
            head = head.get(),
            "the Library already has a head; writing no first head",
        );
        return Ok(None);
    }
    match write(store, keys, policy, Committing::under(policy, &UNWATCHED)).await? {
        Attempted::Committed(committed) => Ok(Some(committed.record)),
        Attempted::Conflict => Ok(None),
    }
}

/// Writes `head-0` with nothing in it, into a slot reserved for it here
/// (spec: CP-2, FM-13).
///
/// The Keyring generation it selects is the empty key table, written and read
/// back like any candidate before the record names it (spec: CP-8, KL-2): a
/// Library that holds no Container maps none.
///
/// [`Attempted::Conflict`] says `head-0` was created by another writer first,
/// which only a Storage that keys objects by name can answer: there both
/// writers aim at the one name. Where Storage mints identifiers the slot is
/// this writer's own, and the create always lands.
pub(super) async fn write(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    policy: &CommitPolicy,
    committing: Committing<'_>,
) -> CommitResult<Attempted> {
    let commitment = keyring::write_generation(
        store,
        keys,
        policy,
        Generation::FIRST,
        &KeyTable::default(),
        committing,
    )
    .await?;
    let aim = journal::first_aim(store).await?;
    let attempted = journal::commit(
        store,
        keys,
        policy,
        aim,
        commitment,
        &PreparedBatch::default(),
    )
    .await?;
    if let Attempted::Committed(_) = &attempted {
        info!("wrote the Library's first head");
    }
    Ok(attempted)
}
