---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0910-1752-check-a-mapped-roots-marker-before-placing-anything-into-it.md
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "EP-13" backend/crates/domain/coffret-usecase/src/in_memory_fs/state/mod.rs && grep -q "EP-13" backend/crates/domain/coffret-usecase/src/fetch/mod.rs && grep -q "EP-13" backend/crates/apps/coffret-device/src/run_fetch.rs && grep -q "RootRefused" backend/crates/apps/coffret-device/src/add/receive_file.rs && grep -q "EP-13" backend/crates/domain/coffret-usecase/src/local_operation.rs && grep -q "RefusedRoot" backend/crates/domain/coffret-usecase/src/unavailable_root.rs && ! grep -rq "planted marker" backend/crates'
assignee: null
branch: task/0911-0330-follow-up-check-a-mapped-roots-marker-before-placing-anything-into-it
created_at: 2026-09-11T03:26:21Z
updated_at: 2026-09-11T15:23:15Z
---

# docs(backend): say where the mapped root's marker is checked, in the docs that lead a reader there

## Overview

The change that added EP-13's placement-time check
(`docs/tasks/2026/0910-1752-check-a-mapped-roots-marker-before-placing-anything-into-it.md`)
kept its file set to the code that performs the check and report it. Six
doc comments that a reader reaches *first* were left describing the world
before it, and one phrase the change made ambiguous is repeated across
five files. This is documentation only: no behaviour, no signature, and no
test may change.

Each item below names the file, what is wrong, and what it should say. Keep
each addition to the voice of the file it lands in, and cite rules the way
its neighbours do.

## What to write

### 1. `coffret-usecase/src/in_memory_fs/state/mod.rs` (the `mod roots;` comment, line 24)

It reads `// What the fake says the filesystem under a mapped root is (spec: EP-12).`,
but `roots.rs` now also holds `State::vouch`, the fake's EP-13 identity
check — the mapped root's *identity*, not the filesystem under it. Name the
second question too, e.g.
`// What the fake says the filesystem under a mapped root is, and whether the root is the one a mapping was recorded against (spec: EP-12, EP-13).`

### 2. `coffret-usecase/src/fetch/mod.rs` (the journey narrative, steps 3 and 7)

Step 3 says the write question is "asked by descending the mapped root the
way step 7 writes into it", and step 7 describes the descent as
component-by-component confinement only. Neither says that the root's
marker is now held against the mapping's expected identity from the
just-opened root handle before any component is descended, nor that a
refused root costs its own mapping and reaches the caller as
`FetchOutcome::refused`. Add one sentence to step 7 and a clause to step 3,
citing EP-13. The mention of "the vouching" among the steps a range read
shares (around line 84) is EP-11's, not EP-13's; make sure a reader can
tell which is which once EP-13 is named.

### 3. `coffret-device/src/run_fetch.rs` (the `OpenLibrary::fetch` doc, lines 26-30)

"A folder is a copy of its part of the Library only where nothing was
surfaced" is now incomplete: a run can surface nothing and still have
placed nothing under a whole mapping. `FetchOutcome`'s own doc already
says "only when `surfaced` and `refused` are both empty (spec: EP-11,
EP-13)"; mirror that here and extend the citation to
`(spec: EP-11, EP-13, KL-7)`.

### 4. `coffret-device/src/add/receive_file.rs` (the `# Errors` section, lines 23-50)

The section enumerates every refusal the call can return —
`UnmappedEntryPath`, `UnmaterializablePath`, `Index`, `Local` — and a
reader takes the list as exhaustive. The call now also returns
`Error::RootRefused`, which `Error::descent` produces from
`DescentError::Refused`. This is the upload entry point EP-13 names by
itself, so add a paragraph beside the `Local` one: the mapped root is not
the folder the mapping was recorded against, this device is placing the one
file it was handed and has no other mapping to go on with, so the request
fails as a whole; nothing was written and nothing was repaired
(spec: EP-13, EP-11).

### 5. `coffret-usecase/src/local_operation.rs` (the `Stating` doc, lines 36-37)

A placement's own open of the mapped root now reports this operation too,
because the descent no longer creates the root. Extend the sentence to name
that second caller, e.g. appending
`— and a placement's own open of one before it vouches for it (spec: EP-13)`.

### 6. `coffret-usecase/src/unavailable_root.rs` (the `UnavailableRoot` doc, lines 5-18)

`refused_root.rs` opens by calling itself "The companion of
`UnavailableRoot`" and spelling out the contrast, and `finding.rs`,
`findings.rs` and `noted.rs` all repeat it — but this doc, the one a reader
lands on first when asking what a mapped-root finding is, never mentions
that a second, differently-shaped verdict about the same root exists. Add
one sentence pointing at `RefusedRoot`: not whether the root is there to be
read from, but whether the folder standing at it is the one whose marker the
mapping recorded (spec: EP-13).

### 7. "a planted marker" now reads as the root marker (five files)

The change made `marker` the reserved word for `.coffret/root`
(`root_marker`, `plant_marker`, `RootRefused::Marker*`), so calling
`plant_other`'s stand-in "a planted marker" reads as the opposite of what
it is. The fake's own modules already use the unambiguous form
(`in_memory_fs/state/roots.rs`, `state/folders.rs`, `state/inspecting.rs`
all say `planted "other"`). Change all five occurrences in one pass, so no
suite is left saying both:

- `coffret-usecase/src/destinations_conformance/root_arrangement.rs:33` (the home of the idiom)
- `coffret-usecase/src/destinations_conformance/blocking.rs:24`
- `coffret-usecase/src/mapped_roots_conformance/folder_arrangement.rs:38`
- `coffret-local-fs/tests/destinations_conformance.rs:11`
- `coffret-local-fs/tests/mapped_roots_conformance.rs:10`

## Out of scope

- Any behaviour, signature, test name, or assertion change.
- Renaming `a_root_with_no_marker_refuses_the_reach` or
  `a_root_with_no_marker_places_nothing_and_reports_the_mapping`, whose
  similarity two refine rounds recorded: the acceptance gates of the parent
  task cite the second name, so a rename is its own change.
- Concept-document additions for the refused root, which the parent task's
  refine rounds recorded as coverage gaps for a separate documentation
  change.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] The fake's `roots` module comment cites EP-13, verified by
      `grep -q "EP-13" backend/crates/domain/coffret-usecase/src/in_memory_fs/state/mod.rs`.
- [x] The fetch journey names EP-13, verified by
      `grep -q "EP-13" backend/crates/domain/coffret-usecase/src/fetch/mod.rs`.
- [x] `OpenLibrary::fetch` names EP-13, verified by
      `grep -q "EP-13" backend/crates/apps/coffret-device/src/run_fetch.rs`.
- [x] `receive_file`'s `# Errors` names the refusal, verified by
      `grep -q "RootRefused" backend/crates/apps/coffret-device/src/add/receive_file.rs`.
- [x] `LocalOperation::Stating` names EP-13, verified by
      `grep -q "EP-13" backend/crates/domain/coffret-usecase/src/local_operation.rs`.
- [x] `UnavailableRoot` points at its companion, verified by
      `grep -q "RefusedRoot" backend/crates/domain/coffret-usecase/src/unavailable_root.rs`.
- [x] No file calls `plant_other`'s stand-in a marker, verified by
      `! grep -rq "planted marker" backend/crates`.

### Manual (reviewer-verified)

- [ ] Each addition reads in the voice of the file it lands in, and no doc
      comment now states a rule twice where one statement and a citation
      would do.
