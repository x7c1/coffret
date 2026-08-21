//! What the policy does with a failure, and what it costs.
//!
//! Every case here runs under a paused clock: `start_paused` makes
//! [`tokio::time`] advance to a sleeper's deadline the moment the runtime has
//! nothing else to do, so a policy that waits two minutes waits none of them in
//! the suite. What the cases then assert about the waits is their *bounds* and
//! their growth. The jitter is drawn at random by design, and a case that
//! pinned an exact wait would be a case about the CSPRNG.

use std::ops::Range;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use coffret_logging::testing::CapturedLogs;
use tokio::time::Instant;
use tracing::Level;

use super::RetryPolicy;
use crate::{ByteStream, CommitSlot, Error, ObjectPage, ObjectRef, ObjectStore, PageToken, Result};

/// The timer ticks in milliseconds, so a wait is served at the first tick at or
/// after its deadline. A bound is therefore a bound to within one tick.
const TICK: Duration = Duration::from_millis(1);

/// A policy that waits in tens of milliseconds instead of seconds.
///
/// Virtual time makes the length of a wait free, but not the arithmetic of
/// reading a case: small round figures are what let the bounds below be read
/// off the page.
fn brisk() -> RetryPolicy {
    RetryPolicy::default()
        .with_attempts(6)
        .with_base_backoff(Duration::from_millis(100))
        .with_wait_ceiling(Duration::from_millis(250))
        .with_total_wait(Duration::from_secs(60))
}

/// A failure a later attempt could still succeed at.
fn worth_retrying(attempt: usize) -> Error {
    Error::Transport {
        detail: format!("the connection dropped on attempt {attempt}"),
    }
}

/// Throttling that names how long to wait.
fn throttled(retry_after: Duration, attempt: usize) -> Error {
    Error::RateLimited {
        retry_after: Some(retry_after),
        detail: format!("slow down, attempt {attempt}"),
    }
}

#[tokio::test(start_paused = true)]
async fn a_failure_worth_retrying_is_followed_by_the_answer_the_next_attempt_gives() {
    let mut calls = 0;
    let answer = brisk()
        .run("get", || {
            calls += 1;
            let failing = calls == 1;
            async move {
                if failing {
                    Err(worth_retrying(1))
                } else {
                    Ok("the object's bytes")
                }
            }
        })
        .await
        .expect("the second attempt succeeds, so the call does");

    assert_eq!(answer, "the object's bytes");
    assert_eq!(calls, 2, "one failure is worth exactly one more attempt");
}

#[tokio::test(start_paused = true)]
async fn a_lost_commit_race_is_answered_at_once_and_costs_no_wait() {
    let started = Instant::now();
    let mut calls = 0;

    let error = brisk()
        .run("put_if_absent", || {
            calls += 1;
            async {
                Err::<(), _>(Error::AlreadyExists {
                    object: "jrn-7.cfrt".to_owned(),
                })
            }
        })
        .await
        .expect_err("a taken slot is not a failure another attempt fixes");

    assert!(matches!(error, Error::AlreadyExists { .. }), "{error:?}");
    assert_eq!(calls, 1, "nothing about the slot changes by asking again");
    assert_eq!(
        started.elapsed(),
        Duration::ZERO,
        "the caller's next move is to refresh the head, not to sleep on it",
    );
}

#[tokio::test(start_paused = true)]
async fn running_out_of_attempts_reports_the_last_failure_the_attempts_produced() {
    let logs = CapturedLogs::capture();
    let mut calls = 0;
    let error = brisk()
        .with_attempts(3)
        .run("list", || {
            calls += 1;
            let attempt = calls;
            async move { Err::<(), _>(worth_retrying(attempt)) }
        })
        .await
        .expect_err("every attempt failed, so the call does");

    assert_eq!(calls, 3);
    // What Storage said last, rather than a wrapper or a synthetic timeout:
    // this is what the caller reports and what the log is read for.
    let Error::Transport { detail } = &error else {
        panic!("the last failure has to survive being given up on: {error:?}");
    };
    assert!(detail.ends_with("attempt 3"), "{detail}");

    // The other half of the field the daily-cap case is recognised by. Pinning
    // only `total_wait` would leave a policy that always reported that bound
    // looking correct, and the two answers are what the field exists to tell
    // apart.
    let event = logs.only(Level::WARN);
    assert_eq!(event.field("bound"), "attempts", "{event}");
    assert_eq!(event.number("attempts"), 3);
}

#[tokio::test(start_paused = true)]
async fn every_wait_is_drawn_from_an_interval_that_doubles_and_then_stops() {
    let policy = brisk();
    // The interval widens while a provider might still be recovering, and stops
    // widening at the ceiling: 100ms, 200ms, then 250ms for as long as the
    // attempts last.
    assert_eq!(policy.interval(0), Duration::from_millis(100));
    assert_eq!(policy.interval(1), Duration::from_millis(200));
    assert_eq!(policy.interval(2), Duration::from_millis(250));
    assert_eq!(policy.interval(9), Duration::from_millis(250));

    let waits = waits_of(&policy, worth_retrying).await;
    assert_eq!(waits.len(), 5, "six attempts are five waits apart");

    for (index, wait) in waits.iter().enumerate() {
        let interval = policy.interval(index as u32);
        assert!(
            *wait <= interval + TICK,
            "wait {index} of {wait:?} fell outside [0, {interval:?}]",
        );
    }
}

#[tokio::test(start_paused = true)]
async fn a_provider_that_says_how_long_to_wait_is_taken_at_its_word() {
    let asked = Duration::from_secs(3);
    let policy = brisk().with_wait_ceiling(Duration::from_secs(30));

    let waits = waits_of(&policy, |attempt| throttled(asked, attempt)).await;

    for wait in &waits {
        assert!(
            *wait >= asked,
            "the provider asked for {asked:?} and got {wait:?}",
        );
    }
}

#[tokio::test(start_paused = true)]
async fn a_wait_a_provider_asks_for_beyond_the_ceiling_is_cut_to_the_ceiling() {
    let ceiling = Duration::from_secs(30);
    let policy = brisk()
        .with_attempts(2)
        .with_wait_ceiling(ceiling)
        .with_total_wait(Duration::from_secs(300));

    let waits = waits_of(&policy, |attempt| {
        throttled(Duration::from_secs(60 * 60), attempt)
    })
    .await;

    assert_eq!(waits.len(), 1);
    assert!(
        waits[0] >= ceiling && waits[0] <= ceiling + TICK,
        "an hour is not a provider's to ask for: {:?}",
        waits[0],
    );
}

#[tokio::test(start_paused = true)]
async fn waiting_stops_at_the_total_ceiling_even_with_attempts_to_spare() {
    let total = Duration::from_secs(3);
    // Throttling that names a whole second each time, so what exhausts the
    // budget is arithmetic rather than the draw.
    let policy = brisk()
        .with_attempts(50)
        .with_wait_ceiling(Duration::from_secs(1))
        .with_total_wait(total);

    let virtually_started = Instant::now();
    let really_started = std::time::Instant::now();
    let mut calls = 0;

    let error = policy
        .run("put", || {
            calls += 1;
            let attempt = calls;
            async move { Err::<(), _>(throttled(Duration::from_secs(1), attempt)) }
        })
        .await
        .expect_err("a refusal that keeps repeating is reported, not waited out");

    assert!(
        calls < 50,
        "the total wait has to stop the loop before the attempts do, and {calls} attempts is not that",
    );
    assert!(
        virtually_started.elapsed() <= total + TICK,
        "the call waited {:?} against a ceiling of {total:?}",
        virtually_started.elapsed(),
    );
    // The clock is paused, so none of that was time anybody spent.
    assert!(
        really_started.elapsed() < Duration::from_secs(1),
        "the suite waited for real: {:?}",
        really_started.elapsed(),
    );

    let Error::RateLimited { detail, .. } = &error else {
        panic!("the last refusal has to survive being given up on: {error:?}");
    };
    assert!(detail.ends_with(&format!("attempt {calls}")), "{detail}");
}

#[tokio::test(start_paused = true)]
async fn an_upload_is_retried_by_handing_the_next_attempt_a_stream_of_its_own() {
    const BODY: &[u8] = b"a Container's ciphertext, all of it";

    let store = FlakyStore::failing_once();
    let mut calls = 0;

    let stored = brisk()
        .run("put", || {
            calls += 1;
            // The caller is what knows how to produce the bytes again — here a
            // buffer, in the transfer flow the spool file it wrote.
            store.put(
                "0123456789abcdef0123456789abcdef.cfrt",
                ByteStream::from(BODY.to_vec()),
            )
        })
        .await
        .expect("the second attempt succeeds, so the upload does");

    assert_eq!(stored.as_str(), "0123456789abcdef0123456789abcdef.cfrt");
    assert_eq!(calls, 2);
    assert_eq!(
        store.stored().as_deref(),
        Some(BODY),
        "the object that finally reached Storage has to hold the whole body",
    );
}

#[tokio::test(start_paused = true)]
async fn giving_up_records_what_it_cost_and_what_it_gave_up_on() {
    let logs = CapturedLogs::capture();
    let policy = brisk()
        .with_attempts(50)
        .with_wait_ceiling(Duration::from_secs(1))
        .with_total_wait(Duration::from_secs(3));

    let _ = policy
        .run("put", || async {
            Err::<(), _>(throttled(Duration::from_secs(1), 1))
        })
        .await;

    let event = logs.only(Level::WARN);
    assert!(event.message().contains("gave up"), "{event}");
    assert_eq!(event.field("operation"), "put");
    assert_eq!(
        event.field("bound"),
        "total_wait",
        "which bound stopped the loop is what tells a daily cap from a bad minute: {event}",
    );
    assert_eq!(event.number("waited_ms"), 3_000);
    // Three waits of the second the provider asked for, and the attempt after
    // the third that found the budget spent.
    assert_eq!(event.number("attempts"), 4);
    // What the provider itself said, rather than how the error renders: the
    // last failure is the evidence this event exists for.
    assert!(
        event.field("error").contains("slow down, attempt"),
        "{event}",
    );
}

#[tokio::test(start_paused = true)]
async fn a_failure_no_attempt_could_fix_is_nobody_s_warning_to_read() {
    let logs = CapturedLogs::capture();

    let _ = brisk()
        .run("put_if_absent", || async {
            Err::<(), _>(Error::AlreadyExists {
                object: "jrn-7.cfrt".to_owned(),
            })
        })
        .await;

    // The policy gave up on nothing: it never tried again, and a lost race is
    // the commit protocol working rather than something to look into.
    assert!(logs.at(Level::WARN).is_empty(), "{}", logs.text());
    assert!(logs.at(Level::ERROR).is_empty(), "{}", logs.text());
}

/// How long the policy waited between attempts, in order.
///
/// Read from the attempts themselves — each one notes when it ran — so what is
/// measured is what the caller actually experienced rather than what the policy
/// says it intended.
async fn waits_of(policy: &RetryPolicy, failure: impl Fn(usize) -> Error) -> Vec<Duration> {
    let mut ran_at = Vec::new();

    let _ = policy
        .run("get", || {
            ran_at.push(Instant::now());
            // Produced here rather than inside the future: the closure is
            // called again for every attempt, and an `async move` block would
            // take `failure` with it the first time.
            let error = failure(ran_at.len());
            async move { Err::<(), _>(error) }
        })
        .await;

    ran_at
        .windows(2)
        .map(|pair| pair[1].duration_since(pair[0]))
        .collect()
}

/// A store whose first uploads fail with something worth retrying.
///
/// It keeps what finally arrived, because the point of retrying a `put` is not
/// that the call returns `Ok` — it is that the object holds the whole body when
/// it does.
struct FlakyStore {
    failures_left: Mutex<u32>,
    stored: Mutex<Option<Vec<u8>>>,
}

impl FlakyStore {
    /// A store that drops the first upload and keeps the next.
    fn failing_once() -> Self {
        Self {
            failures_left: Mutex::new(1),
            stored: Mutex::new(None),
        }
    }

    /// The bytes of the object it holds, if an upload ever finished.
    fn stored(&self) -> Option<Vec<u8>> {
        self.stored.lock().expect("no test panics here").clone()
    }
}

#[async_trait]
impl ObjectStore for FlakyStore {
    async fn put(&self, name: &str, body: ByteStream) -> Result<ObjectRef> {
        // Drained first, so that a dropped upload costs the stream exactly as
        // it would against a real provider: the attempt that failed has already
        // consumed it.
        let bytes = body.into_bytes().await?;
        {
            let mut left = self.failures_left.lock().expect("no test panics here");
            if *left > 0 {
                *left -= 1;
                return Err(Error::Transport {
                    detail: "the connection dropped mid-upload".to_owned(),
                });
            }
        }
        *self.stored.lock().expect("no test panics here") = Some(bytes);
        Ok(ObjectRef::new(name))
    }

    async fn reserve_create(&self) -> Result<CommitSlot> {
        unimplemented!("these cases are about retrying an upload")
    }

    async fn put_if_absent(
        &self,
        _slot: &CommitSlot,
        _name: &str,
        _body: ByteStream,
    ) -> Result<ObjectRef> {
        unimplemented!("these cases are about retrying an upload")
    }

    async fn get(&self, _object: &ObjectRef, _range: Option<Range<u64>>) -> Result<ByteStream> {
        unimplemented!("these cases are about retrying an upload")
    }

    async fn list(&self, _page: Option<&PageToken>) -> Result<ObjectPage> {
        unimplemented!("these cases are about retrying an upload")
    }

    async fn trash(&self, _object: &ObjectRef) -> Result<()> {
        unimplemented!("these cases are about retrying an upload")
    }

    async fn purge(&self, _object: &ObjectRef) -> Result<()> {
        unimplemented!("these cases are about retrying an upload")
    }
}
