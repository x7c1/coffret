---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! test -e backend/crates/apps/coffret-server/src/api_error/redact/not_held.rs'
assignee: null
branch: task/1025-1557-count-keyring-replica-objects-and-let-drop-refusals-redact-themselves
created_at: 2026-10-25T06:57:00Z
updated_at: 2026-10-25T07:23:05Z
---

# fix: count retained Keyring replicas by object, and let a drop refusal redact itself

## Overview

Three small leftovers from the refines of #357 and #360, gathered in one task
because they sit in the same two layers (the commit listing's report and the
server's log rendering) and each is a few lines.

1. **`ControlListing::unnamed_keyring_replicas` counts names, not objects.**
   `backend/crates/domain/coffret-usecase/src/commit/control_listing.rs`
   says "How many listed Keyring replica objects no head names", but adds `1`
   per listed name. Since #360 a name can carry several copies
   (`listed.copies`), so two objects under one name are reported as one. Add
   `listed.copies.len()` instead, so the count is what the doc comment and
   OC-4 say it is. `RetainedKeyringReplicas::replicas`
   (`sync/retained_keyring_replicas.rs`) already documents "How many replica
   objects", so the field's meaning does not change, only the number now
   matches it. This changes what sync reports, which is the point: if an
   existing test pinned the per-name count, update its expectation and say
   why in the PR description. Do not change any spec rule; if a spec passage
   or concept doc says "names" where it means objects, bring the wording to
   the code.
2. **`redact::NotHeld` only exists to wrap `DropRefusal`.**
   `backend/crates/apps/coffret-server/src/api_error/redact/not_held.rs`
   wraps `coffret_device::DropRefusal`
   (`backend/crates/apps/coffret-device/src/drops/drop_refusal.rs`) to
   implement `Redacted` (the trait is in `coffret-model`, and coffret-device
   already implements it for its own `Error` in `src/error/redacted.rs`).
   Implement `Redacted` for `DropRefusal` in coffret-device, following the
   layout the crate already uses for that trait, keep the rendering
   (`DropRefusal::Unknown` / `DropRefusal::Expired`), pass the refusal
   directly at the call site in `api_error/imports.rs` (and wherever else
   `NotHeld` is used), delete `not_held.rs`, and update the module doc in
   `redact/mod.rs` that names `NotHeld`.
3. **The log rendering of a cause a route builds itself is not pinned.**
   `api_error/tests.rs` already pins `record`'s `error = …` field for every
   `ApiError` converted from the device `Error`, through the `recorded()`
   helper. What is not pinned is a cause attached with `.caused_by(…)` by a
   route-level constructor, such as the drop refusal in `api_error/imports.rs`.
   Add tests through the same `recorded()` helper that an import whose drop
   is no longer held records `DropRefusal::Unknown` / `DropRefusal::Expired`,
   and cover the other route-built causes under `api_error/` that have no
   such test yet (`redact::Named`, `redact::NotJoined`, and the like), as far
   as each can be built in a unit test.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `unnamed_keyring_replicas` counts copies; a unit test lists two copies
      under one unnamed Keyring replica name and expects `2` for that
      generation.
- [x] `DropRefusal` implements `Redacted` in coffret-device and
      `redact/not_held.rs` is gone (gate appended); the logged rendering is
      unchanged.
- [x] `api_error/tests.rs` pins the `error` field for the drop refusal and
      for the other route-built causes that lacked a test.
- [x] `make check` passes.
