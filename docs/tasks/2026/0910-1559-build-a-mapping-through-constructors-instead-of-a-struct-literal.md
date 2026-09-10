---
status: completed
pipeline_phase: null
plan: null
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "pub fn new" backend/crates/domain/coffret-usecase/src/device_state/mapping.rs && grep -q "pub fn stamped" backend/crates/domain/coffret-usecase/src/device_state/mapping.rs && ! grep -rn "Mapping {$" backend/crates --include="*.rs" | grep -v KeyringMapping | grep -v "device_state/mapping.rs" | grep -v -- "-> Mapping {" | grep -q .'
assignee: null
branch: task/0910-1559-build-a-mapping-through-constructors-instead-of-a-struct-literal
created_at: 2026-09-10T15:59:10Z
updated_at: 2026-09-10T16:30:10Z
---

# refactor(backend): build a mapping through constructors instead of a struct literal

## Overview

`coffret_usecase::device_state::Mapping`
(`backend/crates/domain/coffret-usecase/src/device_state/mapping.rs`) is built
with a struct literal in some thirty-six places across twenty-four files —
production code (`coffret-device/src/mapping/mod.rs`, `run_catch_up.rs`,
`coffret-sqlite-index/src/rows/device.rs`, the scan modules), conformance
fixtures, and tests in four crates. The next change on this branch adds a
field to `Mapping` (the expected identity of the root's marker, EP-13), and
with literals everywhere that one field touches every one of those files.
Give the type constructors first, so the field can be added in one place.

Add to `Mapping`, in `mapping.rs`:

- `pub fn new(prefix: Option<EntryPath>, local_root: PathBuf) -> Self` — a
  mapping as it is first recorded: no filesystem identity yet (EP-12 says a
  mapping recorded afresh carries none and the next scan stamps what is
  there).
- `pub fn stamped(self, root_identity: RootIdentity) -> Self` — the same
  mapping carrying the identity a scan observed, for the sites that read a
  stamped row back or re-stamp during a scan.

Keep the fields public and the existing derives; a reader may still
destructure. Replace **every** struct-literal construction outside
`mapping.rs` with the constructors (`Mapping::new(..)`, and
`Mapping::new(..).stamped(..)` where an identity is set), including
conformance fixtures, tests, and the row readers in `coffret-sqlite-index`.
Where a site built a mapping with `root_identity: None` and a prefix
`Some(..)` / `None`, the call is `Mapping::new(prefix, local_root)`; do not
add other conveniences (no `root()` / `at()` pair) — two constructors is the
whole surface. Behaviour does not change; every existing test keeps its
assertion.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The constructors exist:
      `grep -q "pub fn new" backend/crates/domain/coffret-usecase/src/device_state/mapping.rs`,
      `grep -q "pub fn stamped" backend/crates/domain/coffret-usecase/src/device_state/mapping.rs`.
- [x] No struct literal of the device-state `Mapping` remains outside its own
      module (a literal opens the brace at end of line; `KeyringMapping` is a
      different type, and a function signature returning `Mapping` is not a
      literal):
      `! grep -rn "Mapping {$" backend/crates --include="*.rs" | grep -v KeyringMapping | grep -v "device_state/mapping.rs" | grep -v -- "-> Mapping {" | grep -q .`.
- [x] Existing backend, frontend, and interoperability checks continue to pass,
      with no test assertion changed.

## Out of scope

Adding the expected-identity field, the marker, the schema bump, or any
behaviour change; those follow on this branch.
