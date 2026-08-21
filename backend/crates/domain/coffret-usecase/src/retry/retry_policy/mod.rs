mod full_jitter;

mod run;

mod wait_after;

use std::time::Duration;

/// How many attempts one call is worth when nothing else is said.
///
/// Enough that a burst of throttling is ridden out rather than reported, and
/// few enough that a Storage provider having a bad minute does not become a
/// worker having a bad hour.
const DEFAULT_ATTEMPTS: u32 = 6;

/// The first wait's upper end when nothing else is said.
const DEFAULT_BASE_BACKOFF: Duration = Duration::from_millis(500);

/// The longest single wait when nothing else is said.
const DEFAULT_WAIT_CEILING: Duration = Duration::from_secs(30);

/// The longest a whole call may spend waiting when nothing else is said.
const DEFAULT_TOTAL_WAIT: Duration = Duration::from_secs(120);

/// How long to keep trying a Storage call that failed with something worth
/// trying again.
///
/// # Why a closure and not a wrapper around the port
///
/// The obvious shape — a store that decorates another and retries inside every
/// method — cannot work for the two operations that matter most.
/// [`put`](crate::ObjectStore::put) and
/// [`put_if_absent`](crate::ObjectStore::put_if_absent) take a
/// [`ByteStream`](crate::ByteStream), and the attempt that failed has already
/// consumed it: a decorator holds nothing it could send a second time. So
/// [`run`](Self::run) takes a closure that *produces* an attempt, and the caller
/// — which is what knows how to open the spool file again — hands each attempt
/// a fresh stream.
///
/// # The four bounds, and what each is protecting against
///
/// - **Attempts** bound the work one call may cost. A failure that repeats is
///   answered rather than retried forever, and the answer is what a re-run
///   starts from.
/// - **The base backoff** is the upper end of the first wait, doubling with
///   each wait after it, so a provider recovering from a moment's overload is
///   not asked again immediately.
/// - **The per-wait ceiling** bounds a single wait, including one a provider
///   asked for by name: `Retry-After` is honoured because the provider knows
///   when it will serve again, and clamped because nothing outside coffret gets
///   to park a worker for an unbounded time.
/// - **The total wait** bounds the sleeping and only the sleeping: how long an
///   attempt itself may take is the attempt limit's to bound, and an upload of
///   several gigabytes takes what it takes. It is also the bound that is
///   load-bearing rather than tidy. Google Drive caps uploads at 750 GB per
///   user per rolling 24 hours and does not publish what the API answers when
///   that cap is hit. If it answers with a throttling reason, an unbounded loop
///   would sit and retry for most of a day while every one of its own logs
///   looked healthy. Bounded waiting turns that into a reported failure that a
///   later run recovers from.
///
/// The two waiting bounds answer different providers, which the defaults make
/// plain: five computed waits come to at most 15.5s, so an ordinary throttling
/// burst is ended by the attempt limit long before the total wait is in
/// question. The total wait is what answers a provider that keeps asking for
/// half a minute by name, and that is the shape the daily cap would arrive in.
///
/// # Full jitter
///
/// Each computed wait is drawn uniformly from zero to
/// `min(ceiling, base * 2^n)` rather than wobbling around the exponential
/// value. The point is to break up retries from workers that failed together:
/// the equal-jitter variant keeps a synchronized floor, and the whole interval
/// is what removes it.
///
/// ```no_run
/// use coffret_usecase::{ByteStream, ObjectStore, RetryPolicy};
///
/// # async fn example(store: &dyn ObjectStore, ciphertext: Vec<u8>) -> coffret_usecase::Result<()> {
/// let object = RetryPolicy::default()
///     // A fresh stream per attempt, from a buffer here.
///     .run("put", || {
///         store.put(
///             "0123456789abcdef0123456789abcdef.cfrt",
///             ByteStream::from(ciphertext.clone()),
///         )
///     })
///     .await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPolicy {
    attempts: u32,
    base_backoff: Duration,
    wait_ceiling: Duration,
    total_wait: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            attempts: DEFAULT_ATTEMPTS,
            base_backoff: DEFAULT_BASE_BACKOFF,
            wait_ceiling: DEFAULT_WAIT_CEILING,
            total_wait: DEFAULT_TOTAL_WAIT,
        }
    }
}

impl RetryPolicy {
    /// Allows a different number of attempts, the first one included.
    ///
    /// # Panics
    ///
    /// If `attempts` is zero: a policy that runs nothing has no answer to give
    /// back, and a caller asking for that has miscounted rather than opted out
    /// of retrying.
    pub fn with_attempts(mut self, attempts: u32) -> Self {
        assert!(attempts >= 1, "a call has to be attempted at least once");
        self.attempts = attempts;
        self
    }

    /// Sets the upper end of the first wait, which doubles from there.
    pub fn with_base_backoff(mut self, base_backoff: Duration) -> Self {
        self.base_backoff = base_backoff;
        self
    }

    /// Sets the longest any single wait may be.
    pub fn with_wait_ceiling(mut self, wait_ceiling: Duration) -> Self {
        self.wait_ceiling = wait_ceiling;
        self
    }

    /// Sets the longest one call may spend waiting in total.
    pub fn with_total_wait(mut self, total_wait: Duration) -> Self {
        self.total_wait = total_wait;
        self
    }
}
