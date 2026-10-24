use coffret_model::{ControlObjectName, KeyTable, KeyringCommitment, ReplicaPosition};

use super::{read_position, ShortKeyring};
use crate::commit::commit_error::{CommitError, CommitResult, UnusableReplica};
use crate::commit::control_keys::ControlKeys;
use crate::commit::control_listing::ControlListing;
use crate::object_store::ObjectStore;
use crate::retry::RetryPolicy;

/// What a read of the committed Keyring came back with.
///
/// The key table is what the caller asked for; the finding beside it is what the
/// walk noticed on the way, and it is carried rather than written down here so
/// that the caller decides when — and whether — the run says it. See
/// [`ShortKeyring`], and [`ShortReport`](super::ShortReport) for what a caller that goes on
/// to write holds it in.
pub(crate) struct ReadKeyring {
    /// The key table the committed generation holds (spec: KL-6).
    pub(crate) key_table: KeyTable,
    /// The set the key table came from, where the walk had to step over a
    /// position to reach it.
    pub(crate) short: Option<ShortKeyring>,
}

impl ReadKeyring {
    /// The key table and the finding, with the finding said now.
    ///
    /// What a flow that only reads does: nothing in such a run will look at the
    /// set again, so the read is the run's one chance to mention it. The
    /// finding comes back as well, for the run's outcome to carry to whoever
    /// asked for it — a log line is what somebody reads afterwards, and the
    /// person who ran the flow is owed it on the spot (spec: KL-15).
    pub(crate) fn reported(self) -> (KeyTable, Option<ShortKeyring>) {
        if let Some(short) = &self.short {
            short.report();
        }
        (self.key_table, self.short)
    }
}

/// The key table the committed Keyring holds, from any one valid replica
/// (spec: KL-1, KL-3, KL-6).
///
/// One valid replica carries the whole logical Keyring, so the replica count is
/// redundancy and never a quorum (spec: KL-6): the first one that reads back
/// valid answers, and the rest are not fetched. A replica Storage will not hand
/// over, one that does not open, or one whose key table is not the one its name
/// promises, is stepped over and the walk goes on to the next position, so a
/// degraded set still serves a read (spec: RV-2). A position whose name Storage
/// lists more than once is read through each copy before it is stepped over,
/// because any copy that reads back valid is the replica (spec: KL-14). Only a
/// generation no replica of answers is refused, and whether that is the Keyring
/// loss RV-7 names is what the reason inside
/// [`CommitError::KeyringUnreadable`] leaves to a caller — which is why that
/// reason keeps a fetch that failed and an object that was rejected apart (see
/// [`UnusableReplica`]).
///
/// A committed replica the walk had to step over is one this read could not
/// take the key table from, and where the object is absent or would not open it
/// is one the set no longer has — fewer valid replicas than the count its
/// commitment selected, which is the degraded state KL-5 names. A read carries
/// on regardless (spec: RV-2), and repairs nothing: restoring the set is a
/// write, and it belongs to the flow that is about to write anyway
/// (spec: KL-11, KL-13), which is [`examine`](super::examine). So the degradation is worth a
/// line, and the line says the set awaits a repair rather than promising one
/// this read performs. A replica Storage merely would not hand over is stepped
/// over too, but nothing about it is known, so it is counted apart and a walk
/// that stepped over only those says so rather than calling the set degraded
/// (spec: KL-15). It is handed back rather than written here, because a
/// caller that goes on to commit has an examination of the same generation in
/// the same run and that examination says its own piece: see
/// [`ShortKeyring`].
///
/// Crate-visible, for the reason [`catch_up`](crate::commit::catch_up()) is: reading the
/// committed Keyring is not the commit's alone. A fetch reads it to open the
/// Containers it pulled back, and a freeze reads it to learn which Containers
/// have no key (spec: KL-7, RV-3) — the same routine, against the same walk of
/// Storage, answering the same rule. Two copies of it would be two readings of
/// KL-1. The commit reads the committed set through [`examine`](super::examine) instead, which
/// is this walk made exhaustive and followed by the repair it is allowed to
/// perform; a commit that also called this one would walk the same set twice
/// for one finding.
pub(crate) async fn read_committed(
    store: &dyn ObjectStore,
    keys: &ControlKeys,
    retry: &RetryPolicy,
    listing: &ControlListing,
    commitment: &KeyringCommitment,
) -> CommitResult<ReadKeyring> {
    let mut last: Option<(u16, UnusableReplica)> = None;
    // The positions stepped over, by what is known about them: a fetch that
    // failed says nothing about the object, and counting it with the ones
    // found lost would report a loss a transient fault made up (spec: KL-15).
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
            Ok(key_table) => {
                return Ok(ReadKeyring {
                    key_table,
                    short: (lost > 0 || unfetched > 0).then(|| {
                        ShortKeyring::new(
                            commitment.generation(),
                            commitment.replica_count(),
                            lost,
                            unfetched,
                        )
                    }),
                });
            }
            Err(cause) => {
                match cause {
                    UnusableReplica::Unfetchable(_) => unfetched += 1,
                    UnusableReplica::Absent
                    | UnusableReplica::Unreadable(_)
                    | UnusableReplica::KindNotAdmitted { .. }
                    | UnusableReplica::DigestMismatch { .. } => lost += 1,
                }
                last = Some((index_of, cause));
            }
        }
    }
    // A commitment declares at least one replica (spec: KL-2), so the walk
    // above always leaves a verdict behind; the fallback is what keeps that
    // invariant from having to be unwrapped.
    let (replica, cause) = last.unwrap_or((0, UnusableReplica::Absent));
    Err(CommitError::KeyringUnreadable {
        generation: commitment.generation(),
        replica,
        cause,
    })
}
