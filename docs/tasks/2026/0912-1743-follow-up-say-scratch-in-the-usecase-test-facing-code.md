---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0912-1420-say-scratch-in-the-usecase-test-facing-code.md
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "local writer" backend/crates/domain/coffret-usecase/src/destinations_conformance/mod.rs'
assignee: null
branch: task/0912-1743-follow-up-say-scratch-in-the-usecase-test-facing-code
created_at: 2026-09-12T17:43:41Z
updated_at: 2026-09-12T18:18:40Z
---

# docs(backend): let the destinations suite name both of its writers

## Overview

`backend/crates/domain/coffret-usecase/src/destinations_conformance/mod.rs`,
lines 3-5. The module doc opens by framing the suite as a fetch's:

```
//! [`Destinations`](crate::Destinations) is how a fetch puts a verified Entry
//! into a folder this device maps, and the rules above it are the ones EP-4 and
//! EP-11 set: ...
```

The capability has two callers, not one. `Destination::create` is reached from
`coffret-usecase/src/fetch/placement.rs` and from
`crates/apps/coffret-device/src/add/incoming_file.rs`, which holds a
`Box<dyn Destination>` and creates its scratch through it. EP-11 scopes
publishing by rename to **a local writer** and names a fetch and an upload as
the two such writers, and `crate::scratch`'s own module doc already says the
reserved prefix is shared by everything that writes into a folder a scan walks.

So the opening sentence states a narrower contract than the suite actually
holds its implementations to: every case below it runs against both callers'
capability. Reword it to say what the suite tests — how a local writer puts a
verified Entry into a folder this device maps — and let the fetch appear as one
of the two writers rather than as the subject. Nothing else in the paragraph
needs to move: the rules it goes on to cite are correct as they stand.

**No behaviour changes.** This is one module doc comment; no code, no test, and
no value the build or the runtime reads.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] the destinations suite's module doc states the contract in terms of a
  local writer rather than a fetch alone:
  `grep -q "local writer" backend/crates/domain/coffret-usecase/src/destinations_conformance/mod.rs`
