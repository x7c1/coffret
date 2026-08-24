use crate::retry::RetryPolicy;

/// How many replicas a newly prepared Keyring generation is written in.
///
/// KL-8 fixes the initial replica policy at three, and a generation's
/// commitment selects the count that generation was prepared under.
const DEFAULT_REPLICA_COUNT: u16 = 3;

/// How many Journal records may accumulate past the newest checkpoint before a
/// commit writes one.
///
/// A trigger and not a bound (spec: CK-8): the record that crosses it, a
/// Snapshot upload that failed, or a single oversized record can each leave
/// more than this much Journal to replay until the next Snapshot lands. The
/// value weighs the replay a stale device would perform against the Snapshot
/// upload that replaces it, and it is a policy parameter rather than a format
/// constant.
const DEFAULT_CHECKPOINT_THRESHOLD: u64 = 64;

/// How many times a commit is rebased onto a new head before giving up.
///
/// Losing the commit slot is the protocol working (spec: CP-3), so a busy
/// Library costs attempts rather than failures. The cap is what stops a device
/// that keeps losing from rebasing forever; the answer at the cap is reported
/// and a later run starts again from the head it reached.
const DEFAULT_ATTEMPTS: u32 = 5;

/// The decisions a commit needs that the Library's state does not make for it.
///
/// Everything else about a commit follows from what is on Storage. These four
/// do not: how many replicas a new Keyring generation is written in (spec:
/// KL-8), how much Journal is allowed to accumulate before a checkpoint is
/// written (spec: CK-8), how many times a losing writer rebases before giving
/// up (spec: CP-4), and how long a single Storage call is worth retrying.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitPolicy {
    /// How many replicas the Keyring generation this commit prepares declares
    /// (spec: KL-8).
    ///
    /// At least one: a generation declaring none can never be complete, so a
    /// commit under a count of zero fails once it has the mapping to commit to
    /// (spec: KL-2).
    pub replica_count: u16,
    /// How many Journal records past the newest checkpoint a commit tolerates
    /// before writing one (spec: CK-8).
    pub checkpoint_threshold: u64,
    /// How many attempts one commit may spend being rebased (spec: CP-4).
    ///
    /// At least one, which is what [`with_attempts`](Self::with_attempts)
    /// enforces and a struct literal has to observe: a commit allowed no attempt
    /// writes nothing and reports having lost every one of them.
    pub attempts: u32,
    /// How long each Storage call is worth retrying.
    pub retry: RetryPolicy,
}

impl Default for CommitPolicy {
    fn default() -> Self {
        Self {
            replica_count: DEFAULT_REPLICA_COUNT,
            checkpoint_threshold: DEFAULT_CHECKPOINT_THRESHOLD,
            attempts: DEFAULT_ATTEMPTS,
            retry: RetryPolicy::default(),
        }
    }
}

impl CommitPolicy {
    /// Declares a different replica count for the generation this commit
    /// prepares.
    pub fn with_replica_count(mut self, replica_count: u16) -> Self {
        self.replica_count = replica_count;
        self
    }

    /// Sets how much Journal may stand past the newest checkpoint.
    pub fn with_checkpoint_threshold(mut self, checkpoint_threshold: u64) -> Self {
        self.checkpoint_threshold = checkpoint_threshold;
        self
    }

    /// Sets how many times a losing writer rebases before giving up.
    ///
    /// # Panics
    ///
    /// If `attempts` is zero: a commit that is never attempted has no answer to
    /// give back.
    pub fn with_attempts(mut self, attempts: u32) -> Self {
        assert!(attempts >= 1, "a commit has to be attempted at least once");
        self.attempts = attempts;
        self
    }

    /// Uses a different retry policy for the Storage calls the flow makes.
    pub fn with_retry(mut self, retry: RetryPolicy) -> Self {
        self.retry = retry;
        self
    }
}
