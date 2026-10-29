use coffret_model::Passphrase;
use coffret_usecase::prune::{prune_journal, PruneOutcome, PruneRequest};
use tracing::info;

use crate::error::Result;
use crate::library_ref::LibraryRef;
use crate::open_library::{open_library, OpenLibrary};

impl OpenLibrary {
    /// Removes the Journal records the Snapshot of the Library's head covers,
    /// all but the head itself, and then the Keyring generations older than the
    /// one that Snapshot records (spec: CK-4, CK-5, CK-6, KL-18).
    ///
    /// The Snapshot is made sure of first — the one another device wrote, or
    /// this device's catalog written into the head's slot (spec: CK-8) — and
    /// nothing is removed unless it records the Keyring tuple the head
    /// committed and that set is complete on Storage; a refusal says which
    /// (spec: CK-5). The records go to the provider's trash, recoverable, and
    /// once they are gone so do the replicas of every superseded Keyring
    /// generation older than the Snapshot's (spec: KL-18). Nothing else on
    /// Storage is touched: no Snapshot, not the Keyring generation the head
    /// selects, no Container (spec: CK-6). A record or a replica the provider
    /// would not trash is reported in the outcome, and the next prune takes it.
    ///
    /// Nothing calls this but a person asking for it: no flow prunes after a
    /// commit or a checkpoint.
    pub async fn prune(&self) -> Result<PruneOutcome> {
        info!(
            operation = "prune",
            library = %self.library_id,
            "pruning the Journal records the head's checkpoint covers"
        );
        Ok(prune_journal(PruneRequest::new(
            self.store.as_ref(),
            self.index.as_ref(),
            &self.keys,
        ))
        .await?)
    }
}

/// Prunes the Journal of the Library `library` names.
///
/// One unlock and one run, which is what a command line does (spec: DK-9). A
/// process that opens a Library once and runs many things over it calls
/// [`OpenLibrary::prune`] and reaches the same body.
pub async fn run_prune<P>(library: &LibraryRef, enter_passphrase: P) -> Result<PruneOutcome>
where
    P: FnOnce() -> Result<Passphrase> + Send,
{
    open_library(library, enter_passphrase).await?.prune().await
}
