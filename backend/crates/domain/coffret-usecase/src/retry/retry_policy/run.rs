use std::future::Future;
use std::time::Duration;

use tokio::time::sleep;
use tracing::warn;

use super::wait_after::retry_after;
use super::RetryPolicy;
use crate::error::{Error, Result};

impl RetryPolicy {
    /// Runs `attempt` until it succeeds, until it fails with something no later
    /// attempt could succeed at, or until one of the bounds is reached.
    ///
    /// The error handed back is the last one the attempts actually produced,
    /// never a synthetic "ran out of attempts": what a caller reports, and what
    /// whoever reads the log has to work from, is what Storage said.
    ///
    /// A failure that is not worth retrying comes straight back, unrecorded and
    /// with nothing waited. That is not only an optimization — a lost
    /// conditional create is [`Error::AlreadyExists`], the commit protocol
    /// working, and the caller's next move is to refresh the control head
    /// rather than to sleep on it.
    ///
    /// `operation` names what was being attempted, for the event emitted if
    /// this gives up; it is the same name the gateways record their own calls
    /// under.
    pub async fn run<T, A, F>(&self, operation: &'static str, mut attempt: A) -> Result<T>
    where
        A: FnMut() -> F,
        F: Future<Output = Result<T>>,
    {
        let mut made = 0;
        let mut waited = Duration::ZERO;
        loop {
            let error = match attempt().await {
                Ok(value) => return Ok(value),
                Err(error) => error,
            };
            made += 1;

            if !error.is_retryable() {
                return Err(error);
            }
            if made >= self.attempts {
                return Err(gave_up(operation, "attempts", made, waited, error));
            }
            let left = self.total_wait.saturating_sub(waited);
            if left.is_zero() {
                return Err(gave_up(operation, "total_wait", made, waited, error));
            }

            // Cut to what is left of the budget rather than overshooting it:
            // the next turn of the loop finds nothing left and gives up, so the
            // ceiling holds however long a single wait wanted to be.
            let wait = self.wait_after(made - 1, retry_after(&error)).min(left);
            waited += wait;
            sleep(wait).await;
        }
    }
}

/// Records that a call ran out of room, and hands back what it failed with.
///
/// `bound` says which room ran out, and telling those two apart is what the
/// event is kept for. Running out of attempts is a provider having a bad minute.
/// Running out of total wait is a provider that kept asking to be left alone,
/// which is the shape Drive's undocumented daily cap would arrive in if it
/// arrives as throttling — so if that ever happens in production, this field is
/// what says it did.
///
/// Nothing on the event is anything but coffret's own accounting and what
/// Storage answered, and an object name is opaque.
fn gave_up(
    operation: &'static str,
    bound: &'static str,
    attempts: u32,
    waited: Duration,
    error: Error,
) -> Error {
    warn!(
        operation,
        attempts,
        bound,
        waited_ms = u64::try_from(waited.as_millis()).unwrap_or(u64::MAX),
        error = %error,
        "gave up: every attempt failed with something worth trying again",
    );
    error
}
