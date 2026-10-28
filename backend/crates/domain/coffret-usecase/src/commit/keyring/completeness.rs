use coffret_model::{ControlObjectName, KeyringCommitment, ReplicaPosition};

use super::{read_position, ShortKeyring};
use crate::commit::commit_error::{CommitResult, UnusableReplica};
use crate::commit::control_keys::ControlKeys;
use crate::commit::control_listing::ControlListing;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// Whether every position the committed set declares reads back valid, and
/// the finding where one does not (spec: KL-1, KL-2).
///
/// The read [`read_committed`](super::read_committed) makes, made exhaustive
/// and with nothing taken from it: that one stops at the first replica that
/// answers, because one valid replica carries the whole Keyring (spec: KL-6),
/// and this one is asked whether the set is *complete* — every position its
/// count declares present and valid — which only a read of every position can
/// say. What asks it is `prune`: once the records a Snapshot covers are gone,
/// the replicas are the envelopes' only carriers, so a set that is short is one
/// whose loss nothing older could make up for (spec: CK-5).
///
/// It repairs nothing. A short set is a writer's to put back, in the
/// examination every commit makes before it writes (spec: KL-11, KL-13); a
/// caller that finds one says so and leaves it. A position Storage would not
/// hand over is counted apart from one found lost, for the reason
/// [`ShortKeyring`] gives — and is short all the same, since what has not been
/// read back has not been shown complete.
///
/// Every position is read whatever an earlier one answered, so the counts are
/// tallies here rather than the floors a read that stops early leaves.
pub(crate) async fn shortfall(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    retry: &RetryPolicy,
    listing: &ControlListing,
    commitment: &KeyringCommitment,
) -> CommitResult<Option<ShortKeyring>> {
    let mut lost = 0u16;
    let mut unfetched = 0u16;
    for index_of in 0..commitment.replica_count() {
        let replica = ReplicaPosition::new(index_of, commitment.replica_count())?;
        let name = ControlObjectName::keyring_replica(
            commitment.generation(),
            commitment.set_digest(),
            replica,
        )?;
        match read_position(store, keys, retry, listing, &name, commitment.set_digest()).await {
            Ok(_) => {}
            Err(UnusableReplica::Unfetchable(_)) => unfetched += 1,
            Err(
                UnusableReplica::Absent
                | UnusableReplica::Unreadable(_)
                | UnusableReplica::KindNotAdmitted { .. }
                | UnusableReplica::DigestMismatch { .. },
            ) => lost += 1,
        }
    }
    Ok((lost > 0 || unfetched > 0).then(|| {
        ShortKeyring::new(
            commitment.generation(),
            commitment.replica_count(),
            lost,
            unfetched,
        )
    }))
}
