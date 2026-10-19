use coffret_model::{EntryPath, Passphrase};
use coffret_usecase::fetch::{
    fetch_entry, Cancellation, EntryFetchOutcome, FetchEntryRequest, FetchMemory, Publication,
    NEVER_CANCELLED, UNHEEDED,
};
use coffret_usecase::{Progress, UNWATCHED};
use tracing::info;

use crate::device_time::now;
use crate::error::Result;
use crate::open_library::{open_library, OpenLibrary};

impl OpenLibrary {
    /// Puts the one Entry at `path` into the folder this device maps it to.
    ///
    /// The same journey [`fetch`](Self::fetch) makes, with the read done
    /// differently: a Container says where everything in it is before any of it
    /// arrives, so one Entry costs the front of the object and the parcels it
    /// overlaps rather than the gigabyte around it (spec: FM-2, FM-9, PK-16,
    /// PK-19). The parcels are kept under this Library's directory until every
    /// Entry they cover that this device maps is placed, and a parcel kept is
    /// never asked of Storage again (spec: PK-21); the other Entries they wholly
    /// cover are placed as they pass, and the outcome names them.
    ///
    /// The answer is one of four, and the third is a finding rather than a
    /// failure: the Entry was placed, it was already here, the run declined
    /// the path and says why (spec: EP-11), or the caller cancelled before the
    /// Entry arrived (spec: PK-21). Beside it is what the run noticed of the
    /// committed Keyring on the way, where the read had to step over a
    /// position of the set (spec: KL-5, KL-15), what else the parcels placed,
    /// and the kept parcels it found not held.
    ///
    /// `progress` is where the run says which phase it is in and whether its one
    /// Container has been read, as [`fetch`](Self::fetch) says it for a folder.
    /// A caller with nowhere to show it passes
    /// [`Unwatched`](coffret_usecase::Unwatched). `cancellation` is asked
    /// before each parcel the run would request from Storage and never inside
    /// one (spec: PK-21); a caller that waits for the Entry passes
    /// [`NEVER_CANCELLED`]. `publication` is told the moment the Entry is
    /// published, before the rest of its parcel has arrived (spec: PK-16); a
    /// caller that waits for the whole run passes [`UNHEEDED`]. Either way
    /// this returns once the whole run is done.
    ///
    /// The catalog is caught up first, every time (spec: CK-9): this is the
    /// fetch of a run that does one thing and ends, and keeps nothing for a
    /// next one.
    ///
    /// Nothing here keeps two callers asking for one Entry from both running it.
    /// A process that serves more than one reader wants
    /// [`EntryFetches`](crate::EntryFetches) around this, which is where that
    /// belongs: it is a property of the process rather than of the Library.
    pub async fn fetch_entry(
        &self,
        path: EntryPath,
        progress: &dyn Progress,
        cancellation: &dyn Cancellation,
        publication: &dyn Publication,
    ) -> Result<EntryFetchOutcome> {
        self.fetch_one(path, progress, cancellation, publication, None)
            .await
    }

    /// The same, answering from the catalog as it stands rather than catching
    /// it up first, and keeping what it learns of Storage for the next one.
    ///
    /// What a process that fetches many Entries over one catalog runs, and
    /// keeps current itself: it catches up as it starts and when somebody asks,
    /// and each of those leaves the listing it walked in
    /// [`fetch_memory`](Self::fetch_memory). A fetch then lists Storage, reads
    /// the committed Keyring, and reads a Container's front only where nothing
    /// kept there is still true (spec: KL-1, FM-2, PK-20). The one thing it
    /// cannot see from here is a stale catalog, and where that shows at all,
    /// what it looks like is a refusal
    /// [`may_mean_catalog_stale`](coffret_usecase::fetch::FetchError::may_mean_catalog_stale):
    /// it is the caller's to catch up and ask once more, because the caller is
    /// who answers for how its catalog stands.
    pub(crate) async fn fetch_entry_as_it_stands(
        &self,
        path: EntryPath,
        cancellation: &dyn Cancellation,
        publication: &dyn Publication,
    ) -> Result<EntryFetchOutcome> {
        self.fetch_one(
            path,
            &UNWATCHED,
            cancellation,
            publication,
            Some(&self.fetch_memory),
        )
        .await
    }

    async fn fetch_one(
        &self,
        path: EntryPath,
        progress: &dyn Progress,
        cancellation: &dyn Cancellation,
        publication: &dyn Publication,
        memory: Option<&FetchMemory>,
    ) -> Result<EntryFetchOutcome> {
        // The Entry Path is not in the event and never will be: it is the user's
        // own name for their file (spec: EL-1), and a log is not where that
        // goes.
        info!(
            operation = "fetch_entry",
            library = %self.library_id,
            catalog = if memory.is_some() { "as it stands" } else { "caught up" },
            "fetching one Entry"
        );
        let request = FetchEntryRequest::new(
            self.store.as_ref(),
            self.index.as_ref(),
            &self.keys,
            self.local_fs.as_ref(),
            self.local_fs.as_ref(),
            self.kept_parcels(),
            path,
            now(),
        )
        .watched_by(progress)
        .cancelled_by(cancellation)
        .heard_by(publication);
        let request = match memory {
            Some(memory) => request.remembering(memory),
            None => request,
        };
        Ok(fetch_entry(request).await?)
    }
}

/// Puts the one Entry at `path` of the Library called `name` into the folder
/// this device maps it to.
///
/// One unlock and one run, which is what a command line does (spec: DK-9). A
/// process that opens a Library once and runs many things over it calls
/// [`OpenLibrary::fetch_entry`] and reaches the same body.
pub async fn run_fetch_entry<P>(
    name: &str,
    enter_passphrase: P,
    path: EntryPath,
    progress: &dyn Progress,
) -> Result<EntryFetchOutcome>
where
    P: FnOnce() -> Result<Passphrase> + Send,
{
    open_library(name, enter_passphrase)
        .await?
        .fetch_entry(path, progress, &NEVER_CANCELLED, &UNHEEDED)
        .await
}
