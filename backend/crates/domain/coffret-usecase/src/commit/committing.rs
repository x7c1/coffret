use crate::commit::commit_policy::CommitPolicy;
use crate::progress::{Phase, Progress, Step};

/// How far one attempt at a commit has got, said in the objects it stores
/// (see [`Phase::Committing`]).
///
/// The total is fixed by the policy before the first object is written. Kept
/// as one value so that every place in the commit that reports reads the same
/// total, and so that none of them says the phase's name or its arithmetic for
/// itself.
#[derive(Clone, Copy)]
pub(super) struct Committing<'a> {
    progress: &'a dyn Progress,
    /// The candidate's replicas.
    replicas: usize,
}

impl<'a> Committing<'a> {
    /// The count a commit under `policy` reports to `progress`.
    pub(super) fn under(policy: &CommitPolicy, progress: &'a dyn Progress) -> Self {
        Self {
            progress,
            replicas: usize::from(policy.replica_count),
        }
    }

    /// Says that an attempt has begun and stored nothing of its own yet.
    ///
    /// Said again by an attempt that rebases, because the candidate the lost
    /// attempt stored is not the one this attempt will commit to (spec: CP-4).
    pub(super) fn begun(self) {
        self.at(0);
    }

    /// Says that `stored` replicas of the candidate Keyring are on Storage.
    pub(super) fn replicas_stored(self, stored: usize) {
        self.at(stored);
    }

    /// Says that the head is written and the batch committed (spec: CP-1).
    pub(super) fn committed(self) {
        self.at(self.total());
    }

    fn at(self, done: usize) {
        self.progress
            .step(Step::new(Phase::Committing, done, self.total()));
    }

    fn total(self) -> usize {
        self.replicas + 1
    }
}
