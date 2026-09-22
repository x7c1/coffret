use coffret_format::ChunkSize;
use coffret_model::{EntryPath, Passphrase};
use coffret_usecase::freeze::{freeze_folder, FreezeOutcome, FreezeRequest};
use coffret_usecase::Progress;
use tracing::info;

use crate::batch_id::{next_batch_id, now};
use crate::error::Result;
use crate::open_library::{open_library, OpenLibrary};

/// How large a Pack comes out by default, in bytes before padding.
///
/// One gibibyte. It lives here rather than in either shell because both of them
/// freeze: the command line takes it as the default of `--target`, and the
/// explorer's server has no flag to take it from at all. Two spellings of one
/// number would be two answers to "how large is a Pack" that nothing keeps in
/// step.
///
/// A default and never a constant of the byte forms: what size serves best is a
/// measurement question about upload and retrieval behaviour, rewrite
/// amplification, object count and provider API overhead (spec: PK-5, PK-6). A
/// Library can be repacked under a different one, so nothing may come to depend
/// on this answer.
pub const DEFAULT_PACK_TARGET: u64 = 1024 * 1024 * 1024;

/// The smallest Pack target a run will take, in bytes before padding.
///
/// One chunk: [`ChunkSize::DEFAULT`] is the plaintext a new Container cuts its
/// body into (spec: FM-6), so a target below it asks for a Pack that cannot
/// fill the first chunk it writes. That is where the floor comes from rather
/// than from anybody's opinion about a sensible size — under it the target has
/// stopped meaning anything the format can act on, and above it the question is
/// the measurement one the target is a parameter for.
///
/// It exists because the flag that carries it is a size in *bytes*, and the
/// number a person thinking in gibibytes types is a single digit. A run under a
/// target of `4` succeeds, cuts one Entry per Pack, and looks exactly like a
/// run that worked (spec: PK-3, PK-4) — so the value is refused where it is
/// read rather than acted on.
pub const MINIMUM_PACK_TARGET: u64 = ChunkSize::DEFAULT.get() as u64;

impl OpenLibrary {
    /// Packs the eligible files under `prefix` into Packs of about `target`
    /// bytes each.
    ///
    /// `prefix` narrows the run to one top-level folder of the Library and never
    /// widens it; `None` is everything the mappings cover (spec: PK-17, EP-9).
    ///
    /// `target` is a parameter and not a constant, and it is one deliberately:
    /// what size serves best is a measurement question about upload and
    /// retrieval behaviour, rewrite amplification, object count and provider API
    /// overhead (spec: PK-5, PK-6). A Library can be repacked under a different
    /// one, so nothing in the byte forms may come to depend on today's answer.
    ///
    /// `progress` is where the run says which Pack it is cutting and which it
    /// is sending, for a caller with somewhere to show it. A process that has
    /// nowhere passes [`Unwatched`](coffret_usecase::Unwatched), and the
    /// reports end there rather than the flow asking who is calling. The
    /// explorer's server is not one of those: each step it is told goes onto
    /// the activity a browser polls.
    ///
    /// The outcome is not a count to glance at: a file whose Entry an existing
    /// Pack holds is reported rather than repacked, and so is one whose Pack the
    /// Library records no key for, so [`Findings`](crate::Findings) over what
    /// comes back is the other half of reading it (spec: PK-14, PK-11).
    pub async fn freeze(
        &self,
        prefix: Option<EntryPath>,
        target: u64,
        progress: &dyn Progress,
    ) -> Result<FreezeOutcome> {
        let now = now();
        let batch = next_batch_id(now);

        // The target is the one decision this run makes that another run of the
        // same folder could make differently, so it is what the log keeps about
        // it. The batch id names the spool an interrupted run leaves behind
        // (spec: OC-2).
        info!(
            operation = "freeze",
            library = %self.library_id,
            batch = %batch,
            target,
            "freezing the mapped folders"
        );
        let mut request = FreezeRequest::new(
            self.store.as_ref(),
            self.index.as_ref(),
            &self.keys,
            self.local_fs.as_ref(),
            self.local_fs.as_ref(),
            &self.spool,
            target,
            batch,
            now,
        )
        .watched_by(progress);
        if let Some(prefix) = prefix {
            request = request.under(prefix);
        }

        Ok(freeze_folder(request).await?)
    }
}

/// Packs the eligible files under `prefix` of the Library called `name` into
/// Packs of about `target` bytes each.
///
/// One unlock and one run, which is what a command line does (spec: DK-9). A
/// process that opens a Library once and runs many things over it calls
/// [`OpenLibrary::freeze`] and reaches the same body.
pub async fn run_freeze<P>(
    name: &str,
    enter_passphrase: P,
    prefix: Option<EntryPath>,
    target: u64,
    progress: &dyn Progress,
) -> Result<FreezeOutcome>
where
    P: FnOnce() -> Result<Passphrase> + Send,
{
    open_library(name, enter_passphrase)
        .await?
        .freeze(prefix, target, progress)
        .await
}
