---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0912-1709-cite-the-rule-each-catalog-removal-follows.md
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check && grep -q "Drops the pending row for one Container, its batch having settled." backend/crates/gateway/coffret-sqlite-index/src/device_state.rs && ! grep -rq "spool row" backend/crates/gateway/coffret-sqlite-index/src/device_state.rs && grep -q "catalog holds the pending rows of (spec: OC-2)." backend/crates/apps/coffret-server/tests/support/mod.rs && ! grep -rq "spool row" backend/crates/apps/coffret-server/tests/support/mod.rs && grep -q "Drops the pending row for one Container, its batch having committed or" backend/crates/domain/coffret-usecase/src/index.rs'
assignee: null
branch: task/0913-0425-follow-up-call-the-device-local-record-a-pending-row
created_at: 2026-09-13T04:22:43Z
updated_at: 2026-09-13T04:36:57Z
---

# docs(backend): call the device-local record a pending row

## Overview

`docs/concepts/index/README.md` defines the device-local record of a Container
awaiting upload as a **pending row**, and `backend/` uses that term in
fifty-six places. Two comments call the same thing a *spool row*. One of them
sits directly opposite the trait method it implements, which uses the
documented term.

The two are all of it. `spool row` appears nowhere else in the repository.

## What to change

### 1. The SQLite catalog's comment, opposite the trait's

`backend/crates/gateway/coffret-sqlite-index/src/device_state.rs:230` documents
`clear_pending_upload`:

```
/// Drops the spool row for one Container, its batch having settled.
```

The trait it implements,
`backend/crates/domain/coffret-usecase/src/index.rs:244`, documents the same
method as:

```
/// Drops the pending row for one Container, its batch having committed or
/// been abandoned.
```

Replace `spool row` with `pending row` in the gateway's line and change nothing
else about it:

```
/// Drops the pending row for one Container, its batch having settled.
```

The rest of the sentence stays. "having settled" and "having committed or been
abandoned" say the same thing at different distances from the flow, and which
of the two reads better is a separate question from what the record is called.

### 2. The server test fixture's comment

`backend/crates/apps/coffret-server/tests/support/mod.rs:310`:

```
            // catalog holds the spool rows of (spec: OC-2).
```

Replace it with:

```
            // catalog holds the pending rows of (spec: OC-2).
```

The plural follows the same rule; the citation and the indentation stay.

## Why the two travel together

Fixing one leaves the other, and the repository then reads as though the two
names were a deliberate distinction rather than a slip. A vocabulary
correction that covers one of two occurrences is worse than either doing all of
it or doing none of it, so this change does all of it — which is two lines.

Neither line is code. Both are comments, and no behaviour changes.

## Acceptance criteria

- `make check` passes.
- `backend/crates/gateway/coffret-sqlite-index/src/device_state.rs` reads
  `Drops the pending row for one Container, its batch having settled.` and
  contains no `spool row`.
- `backend/crates/apps/coffret-server/tests/support/mod.rs` reads
  `catalog holds the pending rows of (spec: OC-2).` and contains no
  `spool row`.
- The trait's own wording at
  `backend/crates/domain/coffret-usecase/src/index.rs` is untouched — the gate
  pins `Drops the pending row for one Container, its batch having committed or`
  so that a sweep cannot rewrite the side that was already right.

## Out of scope

- **`having settled` versus `having committed or been abandoned`.** The
  gateway and the trait describe the same precondition with different words.
  Whether the gateway should adopt the trait's phrasing is a question about
  what the sentence says, not about what the record is called, and it is not
  settled here.
- **Renaming anything in code.** `clear_pending_upload` already carries the
  documented term; no identifier changes.
- **`docs/concepts/index/README.md`.** It is the authority this change defers
  to, and it needs no edit for that.
