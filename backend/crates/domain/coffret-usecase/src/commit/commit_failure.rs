use std::error;
use std::fmt;

use coffret_model::Redacted;

use crate::commit::commit_error::CommitError;
use crate::commit::keyring_repair::KeyringRepair;

/// A commit that did not happen, and the repairs it performed before it
/// stopped (spec: KL-15).
///
/// A run examines the committed Keyring on every attempt, and repairs it where
/// it is degraded, before it writes anything of the batch's own (spec: KL-11,
/// KL-13), so a run can put replicas back on
/// Storage and then fail — the rebase ran out of attempts, the Journal record
/// was refused, the Index could not be refreshed. Those replicas stand whatever
/// became of the batch, and a repair performed is never silent, so the failure
/// carries them for the caller to surface exactly as
/// [`CommitOutcome::repairs`](super::CommitOutcome::repairs) does on success.
///
/// A wrapper rather than a field on every [`CommitError`] variant, because the
/// repairs are the run's and not the refusal's: every step of the flow speaks
/// [`CommitError`] and knows nothing of what earlier attempts put back, and a
/// caller that decides from the variant goes on matching the one it always
/// matched, on [`error`](Self::error).
///
/// To anything walking an error chain it is the commit error itself: its
/// `Display`, its `source`, and its redacted form are the error's own, so a
/// chain reads one sentence per layer with or without it, and nothing about the
/// repairs reaches a diagnostic event through here (spec: EL-1).
#[derive(Debug)]
pub struct CommitFailure {
    /// Why the commit did not happen.
    ///
    /// Boxed, as [`CommitError`] is wherever it travels inside another value,
    /// so that a failure carrying it and its repairs stays the size of a
    /// pointer and a list rather than the size of the largest refusal.
    pub error: Box<CommitError>,
    /// Every repair this run performed before it failed, one per attempt that
    /// put a position back, as on
    /// [`CommitOutcome::repairs`](super::CommitOutcome::repairs).
    ///
    /// Only what an attempt's examination completed: positions an examination
    /// rewrote before it refused the commit travel on
    /// [`CommitError::UnrepairedKeyring`]'s `rewritten` instead, which is about
    /// the generation that refusal names. Empty where the run put nothing back,
    /// which is also every failure raised outside [`commit_batch`](super::commit_batch).
    pub repairs: Vec<KeyringRepair>,
}

impl CommitFailure {
    /// What a person can do about the failure, if there is anything to say
    /// beyond the failure itself — the error's own
    /// [`advice`](CommitError::advice).
    pub fn advice(&self) -> Option<&'static str> {
        self.error.advice()
    }
}

impl From<CommitError> for CommitFailure {
    /// A failure with no repair behind it: every one raised before the commit
    /// flow examined anything, or outside it altogether.
    fn from(error: CommitError) -> Self {
        Self {
            error: Box::new(error),
            repairs: Vec::new(),
        }
    }
}

impl fmt::Display for CommitFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.error, f)
    }
}

impl error::Error for CommitFailure {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        self.error.source()
    }
}

impl Redacted for CommitFailure {
    /// The error's own, and nothing of the repairs: what a log records of a
    /// failed commit is the refusal, and a shell says the repairs out loud.
    fn redacted(&self) -> String {
        self.error.redacted()
    }
}
