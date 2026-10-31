use std::future::Future;

use coffret_device::{EntryFetchOutcome, Error, FetchError};
use tracing::info;

use crate::api_error::ApiError;
use crate::state::ServerState;

use super::run::{self, Logging};

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
    let Some(refusal) = first.as_ref().err().and_then(stale_refusal) else {
        return Ok(first);
    };
    info!(
        operation = "fetch_entry",
        refusal,
        "the Container the catalog names is out of this device's reach, so the catalog is \
         caught up once and the Entry asked for again",
    );
    run::catch_up(state, "fetch", Logging::Not).await?;
    Ok(fetch().await)
}

/// Which of the fetch's two ways of saying the catalog may be stale a refusal
/// is, where it is one of them: `not_found` where Storage says it holds no such
/// object, and `unreachable` where no handle reaches it at all.
///
/// Named in the event because the two are read differently afterwards. The
/// first is the provider's own word; the second is this device having no
/// handle, which the listing it reaches objects through decides.
fn stale_refusal(error: &Error) -> Option<&'static str> {
    let Error::Fetch { cause } = error else {
        return None;
    };
    if !cause.may_mean_catalog_stale() {
        return None;
    }
    match **cause {
        FetchError::ContainerUnreachable { .. } => Some("unreachable"),
        // Storage's `NotFound`, which is the other refusal the gate above
        // admits.
        FetchError::Storage(_) => Some("not_found"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use coffret_device::{FetchError, Missing, StorageError};
    use coffret_model::ContainerId;

    use super::*;

    fn fetch_refused(cause: FetchError) -> Error {
        Error::Fetch {
            cause: Box::new(cause),
        }
    }

    // Both refusals `may_mean_catalog_stale` admits, each named in the event
    // as itself, and a refusal it does not admit named as neither. The route
    // cases meet the first; the second needs a Container no handle reaches,
    // which a store addressed by name never leaves.
    #[test]
    fn each_refusal_that_may_mean_a_stale_catalog_is_named_as_itself() {
        let container_id = ContainerId::from_bytes([7; ContainerId::BYTE_LEN]);
        assert_eq!(
            stale_refusal(&fetch_refused(FetchError::Storage(
                StorageError::NotFound {
                    missing: Missing::Object(container_id.object_name()),
                }
            ))),
            Some("not_found"),
        );
        assert_eq!(
            stale_refusal(&fetch_refused(FetchError::ContainerUnreachable {
                container_id
            })),
            Some("unreachable"),
        );
        assert_eq!(
            stale_refusal(&fetch_refused(FetchError::Storage(
                StorageError::NotFound {
                    missing: Missing::Listing,
                }
            ))),
            None,
        );
    }
}
