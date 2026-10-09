use coffret_model::Redacted;
use coffret_usecase::fetch::let_go_parcels;
use tracing::warn;

use crate::open_library::OpenLibrary;

impl OpenLibrary {
    /// Lets go of every kept parcel that has served its purpose: those of
    /// Containers that left the current set, and those whose mapped Entries
    /// are all on this device or witnessed absent (spec: PK-21).
    ///
    /// What a run that changed the current set without reading a parcel calls
    /// once it has committed — a deletion, or a catch-up. A failure here is
    /// said and not returned: the run it follows has already done what it was
    /// asked, and a parcel left behind costs disk until the next fetch, which
    /// lets the same parcels go on its way in (spec: OC-8).
    pub(crate) async fn let_go_parcels(&self) {
        if let Err(error) = let_go_parcels(self.index.as_ref(), &self.kept_parcels()).await {
            warn!(
                operation = "let_go_parcels",
                library = %self.library_id,
                error = %error.redacted(),
                "kept parcels could not be let go; the next fetch lets them go",
            );
        }
    }
}
