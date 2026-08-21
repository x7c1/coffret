---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make s3-store-it && cd backend && cargo fmt --all -- --check && cargo build && cargo test && cargo clippy --all-targets -- -D warnings"
assignee: null
branch: task/0820-1330-retry-and-limit-classification
created_at: 2026-08-21T05:30:00Z
updated_at: 2026-08-21T09:30:00Z
---

# feat(backend): retry Storage calls with capped backoff and classify provider limits

## Overview

`ObjectStore` already tells a retryable failure from a permanent one by type
(`coffret_usecase::Error::is_retryable`), but nothing acts on it: every call
site would have to write its own loop. This task adds the one retry policy the
transfer flow will drive every Storage call through, and fixes two
classification gaps that a survey of both providers' published limits turned
up.

**Retry belongs to the caller, not to a wrapper around the port.** The obvious
shape — a `Retrying<S: ObjectStore>` decorator implementing `ObjectStore` —
cannot work for the operations that matter most: `put` and `put_if_absent`
take a `ByteStream`, which is consumed by the attempt that fails. A decorator
has nothing left to send on the second try. So the retry helper takes a
closure that *produces* the attempt, and the caller is what knows how to build
a fresh `ByteStream` — by reopening the spool file it wrote. Build it that way
from the start rather than shipping a decorator that silently cannot retry
uploads.

Add a `retry` module to `backend/crates/domain/coffret-usecase` (the crate
already depends on `tokio` for `AsyncRead`, and this is where the errors being
classified live):

- A `RetryPolicy` carrying the maximum number of attempts, the base backoff,
  the per-wait ceiling, and a ceiling on total time spent waiting across one
  call. Give it a `Default` matching what the transfer flow wants, and say in
  the doc comment what each bound is protecting against.
- An entry point that runs an async attempt-producing closure until it
  succeeds, until the error is not retryable, or until a bound is reached, and
  then returns the last error rather than a synthetic one.
- **Full jitter**: each wait is drawn uniformly from zero to
  `min(ceiling, base * 2^attempt)`. Random over the whole interval, not a
  small wobble around the exponential value — the point is to break up
  synchronized retries from parallel workers, and the equal-jitter variant
  keeps a synchronized floor.
- When the provider says how long to wait (`Error::RateLimited::retry_after`),
  honour it in preference to the computed backoff, clamped to the ceiling so
  a provider cannot park a worker for an unbounded time.
- The total-wait ceiling is load-bearing, not decoration. Google Drive caps
  uploads at 750 GB per user per rolling 24 hours, and its published
  documentation does not say what the API answers when that cap is hit. If it
  answers with a throttling reason, an uncapped loop would sit and retry for
  most of a day while looking healthy. Bounded waiting turns that into a
  reported failure that a re-run recovers from, which the flow needs to
  support anyway.

Tests must not actually sleep: drive them with tokio's paused clock
(`#[tokio::test(start_paused = true)]`) so virtual time advances instantly.
Assert the waits by their bounds and their growth, not by exact values — the
jitter is random by design, and a test that pins it is testing the RNG.

Two classification fixes in the gateways, both surfaced by the same survey:

- **`google-drive-store`** answers every non-throttling 403 with
  `Error::PermissionDenied`. Three of Drive's 403 reasons are not permission
  problems and are not throttling either — `storageQuotaExceeded` (the account
  is full), `activeItemCreationLimitExceeded` (500 million items), and
  `numChildrenInNonRootLimitExceeded` (500,000 items in one folder). Today a
  full Drive is reported to the user as "Storage refused access", which sends
  them looking at their OAuth grant instead of at their quota. Add a distinct
  non-retryable `coffret_usecase::Error` variant for a provider limit that is
  reached rather than a permission that is missing, carrying which limit it
  was, and map those reasons to it.
- **`s3-store`** sends every `put` as a single `PutObject`, which S3 caps at
  5 GB. Coffret's normal Packs target 1–2 GiB and stay well under, but an
  oversized singleton Pack is by construction one Entry larger than the target
  (spec: PK-3), so a single very large user file can exceed the cap. Today
  that surfaces as whatever S3's `EntityTooLarge` translates to, after the
  whole body has been streamed. Check the length up front — `ByteStream`
  carries it — and refuse with a typed error naming multipart as what the
  object would need. Use 5,000,000,000 bytes as the threshold: the published
  figure is "5 GB", and erring low costs nothing here.

### What the log is for here

The logging sink landed before this task on purpose. Classification folds the
unknown into known buckets, and the moment `storageQuotaExceeded` gets its own
variant and every other 403 becomes `PermissionDenied`, the fact that an
unfamiliar reason arrived stops existing anywhere. So the events this task adds
are half of its point, not decoration:

- **Giving up** — how many attempts, how long was spent waiting, and the last
  error. The Drive daily-cap case is exactly this shape: the same reason
  repeating until the total-wait ceiling is reached. If that ever happens in
  production, this line is the evidence that says so.
- **A refusal that fell into a catch-all** — already emitted by both gateways.
  Check that the new provider-limit variant does not silently remove a reason
  from that path: a limit that is now classified is one the log no longer has
  to puzzle over, but an unfamiliar 403 must still be recorded verbatim.

Use the existing sink (`coffret-logging` is installed by whoever builds the
program; libraries only emit). The rule it enforces stands: never an Entry Path,
a local file name, plaintext, key material, an OAuth token, or an
`Authorization` header.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A retryable error followed by a success returns the success, and the
      attempt-producing closure was called exactly twice.
- [x] No test compares an error value for equality or through its `Debug` or
      `Display` rendering. The workspace's error types no longer derive
      `PartialEq`; assert on the variant with `matches!` or a `match`, and pin
      a field by destructuring it.
- [x] A non-retryable error (`AlreadyExists`) returns immediately, with the
      closure called exactly once and no wait — a lost commit race must cost
      no delay, because the caller's next move is to refresh the head, not to
      wait.
- [x] Exhausting the attempt limit returns the last error the attempts
      produced, not a wrapper or a generic timeout.
- [x] Every wait falls within `[0, min(ceiling, base * 2^attempt)]`, and the
      upper bound stops growing once it reaches the ceiling.
- [x] `RateLimited` with a `retry_after` waits at least that long, and a
      `retry_after` far beyond the ceiling is clamped to the ceiling rather
      than honoured.
- [x] A stream of retryable errors stops once the total-wait ceiling is
      reached, even when attempts remain, and reports the last error.
- [x] Retrying a `put` works when the caller supplies a fresh `ByteStream` per
      attempt — a test drives the retry helper against a stub store that fails
      once, and asserts the object finally stored holds the complete body.
- [x] The tests above run under a paused clock and the suite does not spend
      real time waiting.
- [x] The Drive adapter maps `storageQuotaExceeded`,
      `activeItemCreationLimitExceeded`, and
      `numChildrenInNonRootLimitExceeded` to the new provider-limit error,
      which reports `is_retryable() == false`; other non-throttling 403
      reasons still map to `PermissionDenied`, and the throttling reasons
      still map to `RateLimited`.
- [x] The S3 adapter refuses a `put` or `put_if_absent` whose `ByteStream`
      declares more than 5,000,000,000 bytes with a typed error, without
      sending a request; a body at or below the threshold is unaffected. The
      MinIO conformance run still passes (`make s3-store-it`).
- [x] The real Drive gateway, driven through the retry policy against the
      scripted transport, recovers from a burst of 429s: the call returns the
      listing, and the transport saw one request per attempt and no more.
- [x] The same holds for throttling Drive dresses as a refusal
      (`userRateLimitExceeded` on a 403), which reaches `RateLimited` by a
      different path than the 429 does.
- [x] A 429 carrying `Retry-After` parks the call for at least the figure
      Drive named, rather than for the far shorter wait the policy would have
      computed.
- [x] A 403 of `storageQuotaExceeded` fails the call at once with the
      provider-limit error, with the transport seeing exactly one request and
      no wait: a full Drive never costs a worker its waiting budget.
- [x] Throttling that outlasts the attempt limit fails the call with what
      Drive answered last and records the `gave_up` warning naming `attempts`
      as the bound that stopped the loop.

## Out of scope

- Multipart upload in the S3 adapter — its own task, and a required one rather
  than a contingency. Libraries this is built for do hold single files past the
  5 GB ceiling — raw video does it on its own — and PK-3 turns each of those
  into an oversized singleton Pack, so the ceiling is reached in normal use.
  Until multipart lands, the typed refusal this task adds is what keeps the
  failure honest and early instead of surfacing after the whole body has been
  streamed. Write the refusal's message and docs accordingly: this is a
  limitation of the adapter today, not a rule about what a Library may contain.
- Determining what Drive answers when the 750 GB daily cap is hit. That needs
  a real account and a large transfer; the bounded waiting this task adds is
  what makes the unknown safe to carry until then.
- Parallel upload workers and the queue that feeds them — they belong with the
  upload pipeline, which is where the concurrency limit has meaning.
- Changing `Error::is_retryable`'s existing verdicts for variants this task
  does not add.
