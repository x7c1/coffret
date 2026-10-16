use std::sync::Arc;

use coffret_model::{KeyTable, KeyringCommitment};
use tracing::debug;

use crate::commit::{read_committed, ControlListing, ShortKeyring};
use crate::fetch::fetch_error::FetchResult;
use crate::fetch::fetch_memory::{FetchMemory, Turn};
use crate::fetch::storage_reads::Origin;
use crate::library_keys::LibraryKeys;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// The committed key table the checkpoint names, what reading it found of the
/// set, and where it came from.
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
///
/// Where another run is reading the set under this commitment already, and
/// began after the last of those forgets, this one waits for that read and
/// takes what it found — the table and the finding, or the refusal. A read
/// dropped before it landed leaves this run to read the set itself.
pub(in crate::fetch) async fn key_table(
    store: &dyn ObjectStore,
    keys: &LibraryKeys,
    retry: &RetryPolicy,
    listing: &ControlListing,
    commitment: &KeyringCommitment,
    memory: Option<&FetchMemory>,
) -> FetchResult<(Arc<KeyTable>, Option<ShortKeyring>, Origin)> {
    let Some(memory) = memory else {
        let (key_table, short) = read_committed(store, keys.control(), retry, listing, commitment)
            .await?
            .reported();
        return Ok((Arc::new(key_table), short, Origin::Read));
    };
    loop {
        // The generation comes with the turn, taken under the same lock, so
        // that a write run of this process repairing the set while this read
        // is under way leaves what it found unkept rather than put back after
        // the repair forgot it (spec: KL-15).
        let (turn, read_under) = memory.key_table_for(commitment);
        let flight = match turn {
            Turn::Kept((key_table, short)) => return Ok((key_table, short, Origin::Kept)),
            Turn::Join(waiter) => match waiter.landed().await {
                Some(landed) => {
                    debug!(
                        read = landed.is_ok(),
                        "the committed key table was being read already, so this run waited for that read",
                    );
                    let (key_table, short) = landed?;
                    return Ok((key_table, short, Origin::Joined));
                }
                None => {
                    debug!(
                        "the read of the committed key table this run waited for was dropped, so it reads the set itself",
                    );
                    continue;
                }
            },
            Turn::Read(flight) => flight,
        };
        let (key_table, short) =
            match read_committed(store, keys.control(), retry, listing, commitment).await {
                Ok(read) => read.reported(),
                Err(error) => {
                    flight.land(Err(error.clone()));
                    return Err(error.into());
                }
            };
        let key_table = Arc::new(key_table);
        memory.remember_key_table(
            read_under,
            commitment.clone(),
            Arc::clone(&key_table),
            short,
        );
        flight.land(Ok((Arc::clone(&key_table), short)));
        return Ok((key_table, short, Origin::Read));
    }
}
