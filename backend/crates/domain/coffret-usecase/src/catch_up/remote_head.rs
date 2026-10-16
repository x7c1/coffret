use std::sync::Arc;

use tracing::debug;

use crate::commit::{CommitResult, ControlListing};
use crate::fetch::FetchMemory;
use crate::index::Index;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// Where the Library's head stands against this device's catalog.
///
/// Two answers and no generations: the question a screen has is whether there
/// is anything to catch up with, and the generations are the Library's own
/// bookkeeping (spec: CK-1), which a catch-up's own line already records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteHead {
    /// Storage names a head newer than the one the catalog stands at: another
    /// device has committed since this one last caught up (spec: CK-9).
    Newer,
    /// Storage names no head newer than the catalog's.
    ///
    /// Including a catalog that stands past every head the listing named,
    /// which is a commit or a catch-up of this device's landing between the
    /// listing and the look at the catalog: there is nothing to catch up with
    /// either way.
    Current,
}

/// Asks Storage, by object names alone, whether the Library has a head this
/// device's catalog has not caught up with.
///
/// One walk of the listing and one look at the catalog's checkpoint, and
/// nothing else: no object is read, so no Keyring, no Container, and no key is
/// reached for — the names say which heads there are (spec: FM-12) — and the
/// catalog is left exactly as it stood. What brings it forward is a catch-up
/// (spec: CK-9), which this deliberately is not.
///
/// The listing is walked before the catalog is looked at, so a catch-up that
/// lands in between is seen as the catalog standing at or past what the
/// listing named, which is [`RemoteHead::Current`] and is true.
///
/// Where the listing names exactly the head the catalog stands at, it is the
/// listing of this catalog, and is kept in `memory` the way a catch-up keeps
/// the one it walks (see [`FetchMemory`]), so the next Entry fetch reaches
/// objects through it rather than listing again. Where it names a newer head it
/// is not this catalog's and is not kept: the catch-up that follows walks its
/// own.
pub async fn remote_head(
    store: &dyn ObjectStore,
    index: &dyn Index,
    retry: &RetryPolicy,
    memory: Option<&FetchMemory>,
) -> CommitResult<RemoteHead> {
    let listing = ControlListing::read(store, retry).await?;
    let newest = listing.newest_head();
    let at = index.checkpoint().await?.map(|at| at.head_generation());

    if newest > at {
        debug!(
            operation = "remote_head",
            catalog = at.map(|at| at.get()),
            newest = newest.map(|newest| newest.get()),
            "the Library has a head this catalog has not caught up with",
        );
        return Ok(RemoteHead::Newer);
    }
    if newest == at {
        if let Some(memory) = memory {
            memory
                .remember_listing(index, Arc::new(listing), at)
                .await?;
        }
    }
    Ok(RemoteHead::Current)
}
