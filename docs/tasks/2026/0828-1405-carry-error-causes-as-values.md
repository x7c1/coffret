---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && (cd backend && RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps --document-private-items) && grep -qF 'Unfetchable' backend/crates/domain/coffret-usecase/src/commit/commit_error.rs && { grep -m1 -A8 'SourceChanged {' backend/crates/domain/coffret-usecase/src/freeze/freeze_error.rs | grep -q 'cause:'; } && git grep -qF 'UntrashedRemoval' -- backend/crates/domain/coffret-usecase/src && ! git grep -qE 'derive\\(.*PartialEq.*\\)' -- backend/crates/domain/coffret-usecase/src/commit/commit_error.rs backend/crates/domain/coffret-usecase/src/freeze/freeze_error.rs"
assignee: null
branch: task/0828-1405-carry-error-causes-as-values
created_at: 2026-08-28T14:05:23Z
updated_at: 2026-08-28T16:25:00Z
---

# fix(backend): carry three error causes as values instead of flattening them

## Overview

Three places decided a caller's question but kept the answer out of the value
it can act on: a verdict that merges "provably invalid" with "could not tell",
an error that names the file but not what moved, and a failure whose reason
survives only in a log line. This task fixes all three. No control-flow
change: every branch keeps making the decision it makes today; what changes is
what the values carry.

**1. `InvalidReplica` merges a verdict with a non-verdict.**
`InvalidReplica::Unreadable(Box<CommitError>)`
(`commit/commit_error.rs:148`–`:170`) covers both "fetching it failed" and
"what came back could not be opened" — its own rustdoc says so. Those are
different findings: a replica whose fetch failed on a Storage fault may be
perfectly intact (the *catalog's* health is unknown), while a replica that
arrived and was rejected is definitively not a usable replica. A caller
reading `CommitError::KeyringUnreadable { cause }` today cannot tell a
degraded Keyring from a flaky provider. Split the variant:

- `Unfetchable(Box<CommitError>)` — the fetch itself failed; Storage did not
  hand the object over, so nothing about the replica's content is known.
- `Unreadable(Box<CommitError>)` — Storage handed the object over and it could
  not be opened (decrypt/decode failure); the replica is definitively bad.

The reader in `commit/keyring.rs` decides which to construct at the point it
already distinguishes the two operations (the `store.get`-side failure vs. the
open-side failure). **Do not change what either reader does with a failure**
— the committed-set reader still steps over and tries the next replica, the
candidate-set reader still stops the commit (spec: KL-6, CP-8, KL-2) — only
the constructed cause becomes precise. Update the `InvalidReplica` rustdoc
(the enum doc and both variants) and its `Display` / `Error::source` impls
(`commit_error.rs:275`–`:293`), and give each new variant the test a caller
matching on it needs (the repo's error-type convention).

**2. `FreezeError::SourceChanged` names the file but drops what moved.**
The variant (`freeze/freeze_error.rs:92`–`:95`) carries only `path`; the
detection sites know more and throw it away: `freeze/spool.rs::moved`
(`:184`–`:193`) and `::closing` (`:196`–`:205`) match on
`coffret_format::Error::{EntryHashMismatch, EntryLengthMismatch,
StreamOverrun}` and discard the matched error, and the pre-stream length probe
(`freeze/spool.rs:108` area) raises the variant directly with no format error
in hand. A person whose freeze stopped can see *which* file moved but not
*how* (length? content? grew past the table?), which is the difference between
"a file was being written" and "something rewrote history". Add a `cause`
field that represents all detection sites honestly — the prescribed shape is a
small `SourceChange` enum in the freeze module (e.g. `LengthMoved { expected,
found }`, `ContentMoved`, `GrewPastTheTable`, or names the code's own
vocabulary suggests), built by `moved` / `closing` from the format error's
fields and by the length probe from its own measurement. Do **not** embed
`coffret_format::Error` itself: the probe site has none, and the format
error's own doc says the encoder's three mismatches "are one thing from here"
— the cause should say what moved, not replay the format layer's spelling.
Extend the variant's rustdoc, `Display`, and the freeze conformance case that
matches on `SourceChanged` so it asserts the cause is the one planted.

**3. `settle::trash_removals` keeps its failure reasons in the log only.**
When trashing a removed Container fails, the reason goes to `warn!` and the
outcome gets a bare `ContainerId` (`commit/settle.rs:61`–`:68`,
`CommitOutcome::untrashed: Vec<ContainerId>` at `commit_outcome.rs:26`). The
checkpoint half of settle is asymmetric in the good direction — its outcome
carries the failure as a value. Introduce
`UntrashedRemoval { container_id: ContainerId, cause: StoreError }` (name and
field per the port's error type; check the actual store error type name) and
make `trash_removals` return `Vec<UntrashedRemoval>`, with
`CommitOutcome::untrashed` following. The rustdoc on the field keeps its
OC-6 citation ("untrashed removal (spec: OC-6)"). Keep the `warn!` line —
logs and values serve different readers. Callers that only counted or listed
IDs adapt mechanically; the sync/commit conformance cases that assert on
`untrashed` assert on the IDs as before and — where a case plants a refusing
store — additionally on the cause being the planted refusal.

Logging rules per `coffret-logging`'s crate doc hold: counts, Container IDs,
object names — never Entry Paths in log fields (the `SourceChanged` cause
travels in the error value, not in a log line). Conventions per `CLAUDE.md`:
no `PartialEq` on error types (match on variants), a test per variant a caller
matches on, English throughout, Conventional Commits, self-contained commit
and PR text, `make check` as the gate.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `InvalidReplica` distinguishes a fetch that failed from an object that
      arrived and was rejected: an `Unfetchable` variant exists next to
      `Unreadable` (check gate on the identifier), both readers in
      `commit/keyring.rs` construct the one matching what actually happened,
      and neither reader's step-over/stop decision changed — the existing
      commit conformance cases still pass unmodified in meaning, and each new
      variant has a unit test matching on it.
- [x] `FreezeError::SourceChanged` carries a `cause` (check gate: the first
      `SourceChanged {` block in `freeze_error.rs` contains a `cause:` field
      within 8 lines — keep the field inside that window) representing every detection site —
      hash mismatch, length mismatch, stream overrun, and the pre-stream
      length probe — and the freeze conformance case that provokes
      `SourceChanged` asserts the planted kind of change comes back.
- [x] `trash_removals` failures reach the outcome as values:
      `UntrashedRemoval` exists (check gate), `CommitOutcome::untrashed`
      carries it, and a conformance case with a store that refuses `trash`
      sees both the Container ID and the refusal cause in the outcome.
- [x] No error type gained `PartialEq` (check gate on both error files), and
      `make check` plus `RUSTDOCFLAGS=-Dwarnings cargo doc --workspace
      --no-deps --document-private-items` are clean.

## Out of scope

- **The `detail: String` convention itself** — replacing string-flattened
  causes across gateway/usecase (`Error::Io { detail }`, the spool-open
  `io::Error` collapse, `RetryPolicy::run`'s error shape) is the next task's
  campaign; this task touches only the three values named above.
- **`upload::verify`'s O(N) listing**, NFC normalization, and every other
  code-side ledger item.
- **Any retry-policy or control-flow change** — which failures are retried,
  stepped over, or fatal stays exactly as it is.
- **`commit/control_listing.rs`'s layering** (it assembling the port-vocabulary
  `MalformedResponse`) — a separate refactor decision.
