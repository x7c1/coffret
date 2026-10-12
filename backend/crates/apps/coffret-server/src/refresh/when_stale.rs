use std::future::Future;

use coffret_device::{EntryFetchOutcome, Error};
use tracing::info;

use crate::api_error::ApiError;
use crate::state::ServerState;

use super::run;

/// One Entry fetch over the catalog as it stands, and one more after catching
/// the catalog up where what the first met says the catalog may be stale.
///
/// A fetch in this process does not catch up by itself: the catalog is caught
/// up as the server starts and whenever somebody asks, and every fetch between
/// answers from it (see [`EntryFetches`](coffret_device::EntryFetches)). Where
/// another device replaced or removed the Container a stale catalog names, the
/// object stays within reach — listed until a run trashes it, and by a handle
/// this device already holds even after — and the fetch answers with the Entry
/// as the catalog shows it. What it meets once the object is out of reach is a
/// Container Storage no longer serves
/// ([`may_mean_catalog_stale`](coffret_device::FetchError::may_mean_catalog_stale)).
/// So the catalog is caught up then, once, through the same run a refresh
/// takes, which keeps how the catalog stands truthful for whoever reads it, and
/// the fetch is asked again: it places the Entry from the Container holding it
/// now, or answers that the Library holds none there any more (spec: CK-9,
/// EP-5).
///
/// Once and no more. A catalog at the head that still meets either refusal has
/// met a Library that does not hold what it says, and that answer is the one to
/// give rather than a reason to go round again.
///
/// The outer refusal is the catch-up's, which is about the Library rather than
/// this Entry; the inner result is the fetch's, as it would have been.
pub(crate) async fn fetch_caught_up_if_stale<F, Fut>(
    state: &ServerState,
    mut fetch: F,
) -> Result<coffret_device::Result<EntryFetchOutcome>, ApiError>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = coffret_device::Result<EntryFetchOutcome>>,
{
    let first = fetch().await;
    if !first.as_ref().is_err_and(says_catalog_stale) {
        return Ok(first);
    }
    info!(
        operation = "fetch_entry",
        "the Container the catalog names is not on Storage, so the catalog is caught up \
         once and the Entry asked for again",
    );
    run::catch_up(state, "fetch").await?;
    Ok(fetch().await)
}

/// Whether a fetch's refusal is the fetch's own way of saying the catalog may
/// be stale.
fn says_catalog_stale(error: &Error) -> bool {
    matches!(error, Error::Fetch { cause } if cause.may_mean_catalog_stale())
}
