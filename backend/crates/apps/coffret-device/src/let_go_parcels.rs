use coffret_model::Redacted;
use coffret_usecase::fetch::{
    let_go_parcels, sweep_unnamed_parcels, tally_kept_parcels, KeptTally,
};
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

    /// Discards every parcel file no held-parcel row names (spec: PK-21).
    ///
    /// For a process that serves fetches to call once as it opens the Library,
    /// after its catch-up and before it serves anything: a fetch in flight is a
    /// row written ahead of its file, which this would otherwise discard the
    /// moment the file appeared. A failure is said and not returned, for the
    /// reason the catch-up's own letting go of parcels gives: what is left
    /// behind costs disk until the next open, and nothing else.
    pub async fn sweep_parcels(&self) {
        if let Err(error) = sweep_unnamed_parcels(self.index.as_ref(), &self.kept_parcels()).await {
            warn!(
                operation = "sweep_parcels",
                library = %self.library_id,
                error = %error.redacted(),
                "the parcel files no row names could not be discarded; the next open tries again",
            );
        }
    }

    /// How many parcels this device keeps for the Library, and how many bytes
    /// their files take (spec: PK-21), or `None` where they could not be
    /// counted.
    ///
    /// A failure is said here and not returned: a count is no fetch, so this
    /// crate's [`Fetch`](crate::Error::Fetch) would put "the fetch did not
    /// finish" over a refusal of a stat, and no other variant of it is this
    /// gesture. What it would have shown is one fact among the Library's
    /// details, and a screen has nothing to do about it.
    pub async fn kept_tally(&self) -> Option<KeptTally> {
        match tally_kept_parcels(self.index.as_ref(), self.local_fs.as_ref()).await {
            Ok(tally) => Some(tally),
            Err(error) => {
                warn!(
                    operation = "kept_parcels",
                    library = %self.library_id,
                    error = %error.redacted(),
                    "the kept parcels could not be counted",
                );
                None
            }
        }
    }
}
