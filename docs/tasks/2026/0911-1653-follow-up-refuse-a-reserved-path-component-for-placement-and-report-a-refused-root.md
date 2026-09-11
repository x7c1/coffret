---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0911-1549-refuse-a-reserved-path-component-for-placement-and-report-a-refused-root.md
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "EP-14" backend/crates/domain/coffret-usecase/src/fetch/mod.rs && grep -q "EP-14" backend/crates/apps/coffret-device/src/run_fetch.rs && grep -q "the names coffret keeps for itself" backend/crates/apps/coffret-device/src/add/mod.rs && grep -q "EP-14" backend/crates/apps/coffret-server/src/routes/upload/mod.rs && ! grep -q "Those three are about the Library" backend/crates/apps/coffret-server/src/routes/upload/mod.rs'
assignee: null
branch: task/0911-1653-follow-up-refuse-a-reserved-path-component-for-placement-and-report-a-refused-root
created_at: 2026-09-11T16:53:00Z
updated_at: 2026-09-11T17:23:20Z
---

# docs(backend): count the reserved name among the refusals the docs enumerate

## Overview

Refusing a path that carries the name reserved for the device's own management
area (spec: EP-14) was added where the refusal is decided and reported. Four doc
comments elsewhere enumerate the refusals a reader should expect, and each was
written before that one existed, so each now undercounts. This is documentation
only: no behaviour, no signature, and no test changes.

### 1. `backend/crates/domain/coffret-usecase/src/fetch/mod.rs` (lines 35-40)

The journey's step 3 lists what becomes a finding rather than a placement, and
the reserved name is missing from the list. Replace the lines beginning
`mapping rather than one Entry (spec: EP-13). Everything else is a finding`
with:

```
//!    mapping rather than one Entry (spec: EP-13). Everything else is a finding
//!    — a path carrying the name reserved for the device's own management
//!    area (spec: EP-14), a file this device never placed, one it placed and no
//!    longer recognizes, a deletion it witnessed, a folder on the way with a
//!    shape no file can be placed through — reported and left untouched, and
//!    the run goes on to the next Entry. Nothing is skipped quietly, which is
//!    the same posture EP-4 takes about never silently selecting one of two
//!    files.
```

### 2. `backend/crates/apps/coffret-device/src/run_fetch.rs` (lines 27-31)

`OpenLibrary::fetch`'s doc says what a declined Entry is, and a path carrying
the reserved name is a second way to be one. Replace the lines beginning
`every Entry the run declined is a path it could not vouch for` with:

```
    /// of the Library only where nothing was surfaced and no mapping was
    /// refused: every Entry the run declined — a path it could not vouch for,
    /// or one carrying the name reserved for the device's own management area
    /// (spec: EP-14) — was left exactly as it was, and every mapping it refused
    /// is a root it placed nothing under at all, so
    /// [`Findings`](crate::Findings) over what comes back is the other half of
    /// reading it (spec: EP-11, EP-13, KL-7).
```

### 3. `backend/crates/apps/coffret-device/src/add/mod.rs` (lines 61-62)

The module comment says the scratch prefix is what an opened mapped root
refuses, which is now half of what it refuses. Replace the two comment lines
beginning `// Opening one, which is where EP-9 is asked, the reserved prefix is
refused, and` with:

```
// Opening one, which is where EP-9 is asked, the names coffret keeps for itself
// are refused, and the descent into the mapped folder is made.
```

### 4. `backend/crates/apps/coffret-server/src/routes/upload/mod.rs` (lines 102-111 and 119-122)

The route's doc counts the refusals settled before a byte reaches disk. It says
three; there are six — the reserved name, the refused mapped root, and a
descent that passes through something that is not a real folder of that root
being the three it never counted. Replace lines 102-111, which begin `Three
refusals, and every one of them is settled before a byte reaches disk.`, with:

```
/// Six refusals, and every one of them is settled before a byte reaches disk.
/// A folder no mapping of this device reaches takes the whole drop with it: there
/// is nowhere to put any of it (spec: EP-9), and the listing has already said so
/// over the rows. A part whose relative path is not an Entry Path is refused by
/// name (spec: EP-2). A part carrying a name coffret keeps for itself — its
/// scratch prefix, or the device's own management area — is refused by name too,
/// because a file written under either would sit in the folder and never reach
/// the Library (spec: EP-11, EP-14). A part whose mapped folder is not the folder
/// its mapping was recorded against is refused as that folder is opened, because
/// nothing is placed into a root that will not vouch for itself (spec: EP-13).
/// A part whose way down from its mapped root passes through something that is
/// not a real folder of that root — a symbolic link, or a file where a folder
/// must be — is refused where the descent meets it, because a file written
/// through it would land somewhere the mappings never named (spec: EP-4, EP-11).
/// And a part standing where the Library holds an Entry inside a Pack is refused
/// by name too, because coffret cannot yet replace one (spec: PK-10, PK-12) —
/// writing it would leave a file in the folder that no flow will ever carry in.
/// An Entry in a Container of its own (spec: PK-15) is not refused: a changed
/// mapped file is eligible for `update`, and replacing the one Container holding
/// it is ordinary work (spec: PK-11, PK-12).
```

and replace lines 119-122, which begin `Those three are about the Library. Three
more are about this server and this`, keeping the sentence that continues the
line after `how many parts there may be.`, with:

```
/// Those six are about the drop itself. Three more are about this server and
/// this device, and they are the three budgets [`Envelope`](crate::Envelope)
/// states: how much one request may carry, how much one part of it may, and how
/// many parts there may be.
```

## Out of scope

- Any behaviour, signature, test name, or assertion change.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] The fetch journey's step 3 counts the reserved name among its findings,
      verified by
      `grep -q "EP-14" backend/crates/domain/coffret-usecase/src/fetch/mod.rs`.
- [x] `OpenLibrary::fetch` names the second way an Entry is declined, verified by
      `grep -q "EP-14" backend/crates/apps/coffret-device/src/run_fetch.rs`.
- [x] The `add` module comment names both reservations, verified by
      `grep -q "the names coffret keeps for itself" backend/crates/apps/coffret-device/src/add/mod.rs`.
- [x] The upload route's doc counts the reserved name among the refusals it
      enumerates, and its stale grouping sentence is gone, verified by
      `grep -q "EP-14" backend/crates/apps/coffret-server/src/routes/upload/mod.rs`
      and
      `! grep -q "Those three are about the Library" backend/crates/apps/coffret-server/src/routes/upload/mod.rs`.
      The count itself must match what the doc actually enumerates; the gate
      deliberately does not pin a number, because the first attempt at this
      criterion pinned one that turned out to be wrong.
