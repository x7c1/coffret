use coffret_model::Generation;
use tracing::warn;

use crate::commit::CaughtUp;
use crate::index::Index;
use crate::sync::sync_error::SyncResult;

/// Keyring replica objects of one generation that no head names: suspected
/// orphans, kept and reported for a person to look at (spec: OC-4).
///
/// What a commit interrupted after it put its candidate's replicas and before
/// its head leaves, most of the time. Nothing proves it — a commit another
/// device is making right now has put its replicas and not yet its head — and
/// nothing here removes it or reads it: reporting is all the rule allows
/// (spec: OC-4).
///
/// A generation and a count, and never an object name: the generation is the
/// Library's own bookkeeping and the count is how much of it there is, which
/// is all a reader needs to weigh it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetainedKeyringReplicas {
    /// The Keyring generation the replica names state.
    pub generation: Generation,
    /// How many replica objects of that generation no head names.
    pub replicas: usize,
}

/// The Keyring replica objects the catch-up's listing names that no head
/// names, by generation, measured against the Keyring the catalog's head
/// commits (spec: OC-4).
///
/// Read before the run writes a candidate of its own, so what is counted was
/// left by somebody else, or by an earlier run of this device's.
pub(crate) async fn retained_keyring_replicas(
    caught: &CaughtUp,
    index: &dyn Index,
) -> SyncResult<Vec<RetainedKeyringReplicas>> {
    let committed = index.checkpoint().await?;
    let retained: Vec<RetainedKeyringReplicas> = caught
        .listing
        .unnamed_keyring_replicas(committed.as_ref().map(|at| at.keyring()))
        .into_iter()
        .map(|(generation, replicas)| RetainedKeyringReplicas {
            generation,
            replicas,
        })
        .collect();
    for one in &retained {
        warn!(
            generation = one.generation.get(),
            replicas = one.replicas,
            "retained Keyring replicas no head names",
        );
    }
    Ok(retained)
}
