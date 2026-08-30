use coffret_model::EntryPath;
use coffret_usecase::fetch::{fetch_entry, EntryFetch, FetchEntryRequest};
use tracing::info;

use crate::batch_id::now;
use crate::error::Result;
use crate::open_library::open_library;

/// Puts the one Entry at `path` of the Library called `name` into the folder
/// this device maps it to.
///
/// The same journey [`run_fetch`](crate::run_fetch) makes, with the read done
/// differently: a Container says where everything in it is before any of it
/// arrives, so one Entry costs the front of the object and the chunks covering
/// that Entry rather than the gigabyte around it (spec: FM-2, FM-5, FM-9).
///
/// Per PK-16 that is an optimization inside fetching the containing Container
/// and not a fetch unit of its own: the rest of the Container is exactly as
/// unfetched afterwards. So this is what a reader wanting one page uses, and
/// never what a device bringing a folder into line uses.
///
/// The answer is one of three, and the third is a finding rather than a failure:
/// the Entry was placed, it was already here, or the run declined the path and
/// says why (spec: EP-11).
pub async fn run_fetch_entry<P>(
    name: &str,
    enter_passphrase: P,
    path: EntryPath,
) -> Result<EntryFetch>
where
    P: FnOnce() -> Result<Vec<u8>> + Send,
{
    let library = open_library(name, enter_passphrase).await?;

    // The Entry Path is not in the event and never will be: it is the user's own
    // name for their file (spec: EP-1), and a log is not where that goes.
    info!(
        operation = "run_fetch_entry",
        library = %library.library_id,
        "fetching one Entry"
    );
    Ok(fetch_entry(FetchEntryRequest::new(
        library.store.as_ref(),
        library.index.as_ref(),
        &library.keys,
        path,
        now(),
    ))
    .await?)
}
