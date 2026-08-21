---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design]
max_refine_rounds: 2
retries_remaining: 1
check_command: "make deps && cd backend && cargo fmt --all -- --check && cargo build && cargo test && cargo clippy --all-targets -- -D warnings"
assignee: null
branch: task/0820-1607-logging-to-a-rotating-file
created_at: 2026-08-20T16:07:45Z
updated_at: 2026-08-21T06:10:00Z
---

# feat(backend): record what Storage actually answered, to a size-bounded log file

## Overview

Nothing in this workspace logs. There is no logging dependency, no `info!` or
`warn!` anywhere, and no way for a person to find out afterwards what a Storage
provider actually returned. That is a problem specific to what coffret is: it
depends on third-party APIs whose real behavior is not fully knowable from
their documentation, and it has already been bitten by that more than once.

- Google Drive caps uploads at 750 GB per user per rolling 24 hours. The cap is
  documented; **what the API answers when you reach it is not**. If it answers
  with a throttling reason, a retry loop will sit on it for most of a day
  looking healthy.
- `files.generateIds` does not document whether the ids it mints expire.
- The suite's own documentation asserted that a `drive.file` grant cannot name
  a web-interface folder as a parent. Running it against a real account
  disproved that, and the wrong claim had stood in the repository until then.

So the first purpose of logging here is not general debugging. It is to keep
**evidence of what really came back**, in a form a person can read later and
decide from: "production answered X, so the code should handle X."

That purpose fixes the ordering. This task lands **before** the error
classification work, because classification folds the unknown into known
buckets: the moment `storageQuotaExceeded` gets its own variant and every other
403 becomes `PermissionDenied`, the fact that an unfamiliar reason arrived stops
existing anywhere. The path that records it has to exist first, or the only
thing that ships is the code that discards it.

What this task settles is stated here in full, so it is self-contained.

### What to build

- Add `tracing` as the logging facade, and a subscriber configured by whoever
  builds the application — not by a library crate. Library crates emit events
  and never install a subscriber, so a test or an embedding binary with no
  subscriber simply sees nothing emitted and behaves identically.
- Write to a **file**, not just stderr: this is a local application whose user
  is not watching a terminal, and the whole point is to be able to investigate
  afterwards. Default location `${XDG_STATE_HOME:-$HOME/.local/state}/coffret/logs/`
  — state, because losing it loses the evidence, and it is neither cache nor
  configuration. Create it with mode 0600, as the token cache is.
- **Write JSONL** — one JSON object per line. What this log is for is analysis,
  not reading a terminal: the questions it has to answer are aggregate ones, and
  `tracing` events already carry named fields that the human-readable formatter
  flattens into a message tail. JSON keeps that structure, and escapes the
  quotes, braces and newlines that a verbatim provider response body brings with
  it. It costs bytes against the ceiling, so measure the cost rather than
  assuming it, and record the number.
- **Rotate with a ceiling on total bytes, not on file count.** The requirement
  is that logging can never grow without bound on disk. Rotating daily and
  keeping N files does not satisfy it: nothing bounds how much a single day
  writes. Note that `tracing-appender`'s `RollingFileAppender` rotates by period
  and its `max_log_files` bounds the number of files, not their size — so it
  does not meet the requirement on its own. Pick an approach that does, and say
  in the code why. Oldest files are the ones to drop: recent failures are the
  evidence worth keeping.

### What to record

The events below are what this task asks for. Add them where the code already
knows the answer — this task wires up logging and the events that need no new
plumbing; it does not restructure any call path to make an event possible.

| what | level |
| --- | --- |
| A provider answer that fell into a catch-all rather than a known state — `Rejected`, `PermissionDenied`, `MalformedResponse`. Record the operation, the HTTP status, the provider's reason, and the response body as it arrived | `warn` |
| Giving up after retries, with how many attempts and how long was spent waiting | `warn` |
| A result that is typed as success but contradicts an assumption — an `AlreadyExists` outside a commit race, for one | `warn` |
| Ordinary progress: a batch committed, objects uploaded | `info` |
| Individual HTTP calls | `debug` |

`NotFound` is **not** an error-level event. Looking for a control object that is
not there is ordinary, and the level convention reserves `error` for a failure
the user has to act on.

A log line does not satisfy a user-facing obligation. `KL-15` requires Keyring
replica loss and repair to reach the user as a health event; logging them is not
that, and this task does not claim to implement it.

### What must never be logged

This is not a style rule. coffret hides the user's folder structure from the
Storage provider behind opaque object names; writing it into a plaintext log on
the same disk would open the exact leak the design closes, outside the reach of
whole-disk encryption.

Never: Entry Paths or local file names; plaintext contents or fragments of them;
the Master Key, Container Keys, purpose keys, Key Envelopes, the Passphrase, a
Recovery Code; OAuth access or refresh tokens and the `Authorization` header.

Safe, and useful: a Container's object name — an opaque random id (FM-3) that
says nothing on its own; a control object's name (`jrn-7.cfrt` and the like),
whose kind and update frequency are already accepted leakage (FM-12); Container
IDs, generations, ciphertext sizes and hashes; HTTP status, provider reason
strings, retry counts.

The layer boundary carries the rule: the Storage layer handles opaque values and
may record freely, while any layer holding Entry Paths may not. Recording
provider response bodies verbatim is safe for the same reason — the names sent
to the provider are opaque. Where a response body could carry something from the
"never" list, redact it rather than dropping the whole event.

### A constraint that will fail the build if missed

**`coffret-model` must not gain a logging dependency.** It is asserted to have
zero third-party dependencies and `make deps` enforces it; adding `tracing`
there turns CI red. That constraint is deliberate. `coffret-format` is pure
computation with no outside interaction worth recording. Logging belongs to the
gateway crates and the use-case layer.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make deps` still passes — `coffret-model` has no new dependency.
- [x] Emitting events without a subscriber installed changes nothing: a test
      exercises a code path that logs and asserts its result is unaffected.
- [x] The rotation implementation enforces a **total size** ceiling: a test
      writes past the configured ceiling and asserts that the bytes on disk
      afterwards stay at or below it, and that the oldest content is what went.
- [x] The log file is created with mode 0600.
- [x] A gateway response that maps to a catch-all error emits a `warn` event
      carrying the status, the reason, and the body — asserted with a capturing
      subscriber against the existing stub transport, not against a live API.
- [x] A `NotFound` does not emit at `error` level.
- [x] No event carries anything from the "never" list. Back this with a test
      that logs through a path handling a value from each category available at
      that layer, and asserts the captured output does not contain it.

### Manual / on-hardware (verified by a human before merge)

- [x] After a real `drive-store-it` run, the log file exists at the documented
      path, is readable, and contains the run's Storage calls at the expected
      levels — with no Entry Path, token, or key material anywhere in it.

## Out of scope

- Reworking `Error` variants, their fields, or their `is_retryable` verdicts —
  including the classification gaps the next task fixes. This task records what
  arrives; it does not change how anything is categorised.
- The retry policy itself. Its events belong to it and land with it; what this
  task provides is the facade and the sink they will use.
- Surfacing health events to the user (`KL-15`).
- Metrics, tracing spans exported anywhere, or any network destination. The
  sink is a local file and nothing else.
- Instrumenting the frontend.
