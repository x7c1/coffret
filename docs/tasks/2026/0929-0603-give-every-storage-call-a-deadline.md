---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check && git grep -qE "operation_timeout|TimeoutConfig" -- backend/crates'
assignee: null
branch: task/0929-0603-give-every-storage-call-a-deadline
created_at: 2026-09-29T06:03:00Z
updated_at: 2026-09-29T09:28:17Z
---

# fix(backend): give every Storage call a deadline it states

## Overview

The Storage gateways have no per-call timeout policy, so a call can stall the
CLI's `sync` / `fetch` or the server's refresh indefinitely:

- **Drive** (`backend/crates/gateway/google-drive-store/src/http/reqwest_transport.rs`,
  lines 17–24) sets a 20 s connect timeout and a 60 s *between-bytes* read
  timeout, and deliberately no whole-call timeout, because a large Container
  upload is legitimately slow. That reasoning holds for calls that stream a
  body, but it also leaves the small calls unbounded: an answer that trickles one
  byte at a time, or a list / metadata call that answers without progressing,
  never ends.
- **S3** (`backend/crates/apps/coffret-device/src/s3.rs`, lines 26–44) sets no
  timeouts and inherits the SDK's defaults, which depend on `BehaviorVersion`
  and are stated nowhere in this repository.

Meanwhile `RetryPolicy` already treats a timeout as `Timeout` and retries it, so
the retry machinery assumes a deadline that the small calls do not have.

Split the policy by the kind of call:

- Calls that **stream a body** (`put` / `get` of objects): keep the current
  between-bytes timeout; no whole-call deadline.
- Calls whose body is **small** (listing, metadata, the conditional create
  `reserve_create` / `put_if_absent`, trash / delete, the join and bucket
  probes): a whole-call deadline in addition.
- **S3**: state the timeouts explicitly where the client is built — a
  `TimeoutConfig` (connect, operation, operation attempt) and the SDK's
  stalled-stream protection — with values that match Drive's or a comment
  saying why they differ. Where the SDK cannot tell the two kinds of call apart
  by one client config, apply the whole-call deadline per operation for the
  small calls.
- Write the numbers down once, as named constants with doc comments that say
  what each bounds and why, and state how they relate to the explorer's 60 s
  start-up catch-up wait (the e2e wait was aligned with it earlier). If the
  observability or transfer design in the repository describes timeouts, update
  it to match.
- A deadline that fires must surface as the same retryable `Timeout`
  classification the transports use today, so `RetryPolicy` needs no change;
  confirm that and pin it in a test.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Drive: a transport test against a local fake server that accepts the
      connection and never answers a small call (e.g. a files listing) fails with
      the `Timeout` classification within the configured deadline (the test uses
      a shortened deadline, not the production value).
- [x] Drive: a transport test against a fake server that dribbles a small call's
      answer one byte at a time under the between-bytes timeout still fails with
      `Timeout` once the whole-call deadline passes.
- [x] Drive: a streamed `get` / `put` whose bytes keep flowing is not cut off by
      the whole-call deadline (test with a shortened deadline shorter than the
      transfer).
- [x] S3: the client is built with an explicit timeout configuration (gate
      appended to `check_command` looks for `operation_timeout` or
      `TimeoutConfig` in `backend/crates`), and an s3-store test against a
      loopback stub that never answers a small call fails as `Timeout` within
      the configured deadline.
- [x] A test pins that the `Timeout` produced by a fired deadline is one
      `RetryPolicy` retries.

### Manual / on-hardware (verified by a human before merge)

- [ ] CI's `s3-store` and `e2e` jobs (MinIO) are green on the PR.
- [ ] `make drive-round-trip-it` is green against real Drive with the new
      deadlines (no legitimate call is cut off).
