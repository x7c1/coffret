use coffret_usecase::sync::{sync_folders, SyncOutcome, SyncRequest};
use tracing::info;

use crate::batch_id::{next_batch_id, now};
use crate::error::Result;
use crate::open_library::open_library;

/// Carries the folders this device maps into the Library called `name`.
///
/// The whole of what this adds to [`sync_folders`] is composition: the Library
/// is opened, the two values a device supplies rather than derives — what it
/// calls this batch and what its clock says — are drawn here, and the flow runs
/// under the default policy. Which folders are scanned is not among the
/// arguments, and cannot be: that is the device's mappings, which the catalog
/// holds (spec: EP-9).
///
/// The outcome is not a count to glance at. A run that returns `Ok` has not
/// necessarily backed everything up — a file whose Entry lives in a Pack and a
/// mapped root the device could not vouch for are both reported rather than
/// acted on — so [`Findings`](crate::Findings) over what comes back is the other
/// half of reading it (spec: PK-14, EP-12).
pub async fn run_sync<P>(name: &str, enter_passphrase: P) -> Result<SyncOutcome>
where
    P: FnOnce() -> Result<Vec<u8>> + Send,
{
    let library = open_library(name, enter_passphrase).await?;
    let now = now();
    let batch = next_batch_id(now);

    // The batch id names the spool directory an interrupted run leaves behind
    // (spec: OC-2), so it is worth having in the log of the run that made it.
    // It is opaque and names nothing outside this device.
    info!(
        operation = "run_sync",
        library = %library.library_id,
        batch = %batch,
        "syncing the mapped folders"
    );
    Ok(sync_folders(SyncRequest::new(
        library.store.as_ref(),
        library.index.as_ref(),
        &library.keys,
        &library.spool,
        batch,
        now,
    ))
    .await?)
}

#[cfg(test)]
mod tests {
    use super::run_sync;
    use crate::error::Error;
    use crate::testing::state_dir;

    // Every one of these flows opens the Library first, and opening one reads
    // the settings before it asks for anything: a Library that is not on this
    // device is refused without a Passphrase. The case is here rather than four
    // times over because `run_freeze`, `run_fetch` and `run_fetch_entry` reach
    // the same call — a mistyped `--library` costs nobody a Passphrase, and a
    // script is told which Library is missing rather than told that its empty
    // Passphrase protects nothing.
    #[tokio::test]
    async fn a_library_that_is_not_here_is_refused_before_a_passphrase_is_read() {
        state_dir();
        let unasked = || panic!("no Passphrase may be asked for before a refusal that needs none");

        let result = run_sync("nothing-of-that-name", unasked).await;
        assert!(
            matches!(&result, Err(Error::NoSuchLibrary { name, .. }) if name == "nothing-of-that-name"),
            "expected the name to be refused, got {result:?}"
        );
    }
}
