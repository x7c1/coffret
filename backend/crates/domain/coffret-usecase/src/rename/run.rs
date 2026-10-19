use tracing::info;

use crate::commit::{catch_up, commit_batch, CommitRequest, PreparedBatch};
use crate::progress::{Phase, Step};
use crate::relocate::{relocate_waiting, Relocation};
use crate::rename::plan::{self, Plan};
use crate::rename::rename_error::{RenameError, RenameResult};
use crate::rename::rename_outcome::RenameOutcome;
use crate::rename::rename_request::RenameRequest;
use crate::sync::{complete_after_catch_up, PendingWork};

/// Renames an Entry, or a folder of Entries, in one Journal record, and uploads
/// nothing (spec: FM-15, CP-1).
///
/// The steps, and the rule each answers to, are listed in the
/// [module documentation](crate::rename).
///
/// A refusal is [`RenameError::Refused`], and comes before anything is written.
/// A commit another device made between the plan and the commit is rebased onto
/// by the rules EP-7 gives a pending rename: one of the same Entry, or one that
/// took a destination, is [`RebaseConflict`](crate::commit::CommitError::RebaseConflict),
/// which a person answers by running the rename again.
///
/// Nothing here reads, encrypts or uploads an Entry's content, or writes a
/// Container. The only objects stored are the ones every commit stores: the
/// record, and the Keyring replicas and the Snapshot where the commit is due to
/// write them (spec: CP-8, CK-8).
pub async fn rename_entries(request: RenameRequest<'_>) -> RenameResult<RenameOutcome> {
    let RenameRequest {
        store,
        index,
        keys,
        spool,
        from,
        to,
        progress,
        policy,
        now,
        disk,
    } = request;

    plan::refuse_outright(&from, &to)?;

    progress.step(Step::begun(Phase::CatchingUp));
    let caught = catch_up(store, index, keys.control(), &policy.retry).await?;
    // A batch of this device's that the catch-up found committed is this
    // device's own before the plan, so the files it carried in have the
    // materialization records the relocation after the commit follows them by.
    // Left for a later completion, they would stay at the paths the batch
    // brought them in at, and be recorded there (spec: OC-7, EP-10).
    complete_after_catch_up(index, spool, now, PendingWork::Unowned).await;
    // What the plan reads the catalog at, and so what the batch is prepared
    // against (spec: CP-4, EP-7).
    let base = caught.head_generation();
    let Plan {
        renames,
        renamed,
        folder,
    } = plan::plan(index, &from, &to).await?;

    let batch = PreparedBatch::default().renaming(renames).prepared_at(base);
    let commit = commit_batch(
        CommitRequest::new(store, index, keys.control(), batch)
            .with_policy(policy)
            .watched_by(progress),
    )
    .await?;

    // Only once the commit has landed: before it, each Entry stands at its old
    // path, and the file there is an ordinary materialized one (spec: EP-10).
    let relocations: Vec<Relocation> = match disk {
        Some(disk) => {
            relocate_waiting(index, disk)
                .await
                .map_err(|cause| RenameError::NotRelocated {
                    generation: commit.record.generation(),
                    cause,
                })?
        }
        None => Vec::new(),
    };

    // Counted rather than named: the paths are the user's own names for their
    // files (spec: EL-1).
    info!(
        renamed = renamed.len(),
        folder,
        head = commit.record.generation().get(),
        attempts = commit.attempts,
        relocated = relocations
            .iter()
            .filter(|relocation| !matches!(relocation, Relocation::Refused { .. }))
            .count(),
        relocation_refused = relocations
            .iter()
            .filter(|relocation| matches!(relocation, Relocation::Refused { .. }))
            .count(),
        "a rename finished",
    );
    Ok(RenameOutcome {
        renamed,
        folder,
        commit,
        relocations,
    })
}
