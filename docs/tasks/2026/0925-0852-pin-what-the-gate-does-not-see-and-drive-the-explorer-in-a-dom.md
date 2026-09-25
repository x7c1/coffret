---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/0925-0852-pin-what-the-gate-does-not-see-and-drive-the-explorer-in-a-dom
created_at: 2026-09-25T08:52:52Z
updated_at: 2026-09-25T11:55:44Z
---

# test: pin what the gate does not see, and drive the explorer's hooks in a DOM

## Overview

Tests the last several changes could not write, and blind spots in the gate
itself. Each item names the test that is missing and the double or dependency
it needs; nothing here changes what the code does, except where a bound has
to exist for a test to pin it.

### 1. The explorer's hooks and handlers have no DOM to run in

The web package has no `jsdom` and no Testing Library, so a hook's state
across renders and a click or drop handler's argument cannot be exercised;
two changes in a row dropped tests for that reason. Add `jsdom`,
`@testing-library/react` and `@testing-library/dom` as dev dependencies of
`frontend/packages/apps/web`, set Vitest's environment to `jsdom` for that
package, and write the tests that were dropped: the value `onUnmapped` is
called with when a file row in an unmapped folder is clicked and when a
folder is dropped on an unmapped root; `useActivity`'s `trouble` being kept
across a poll and cleared by the path that should clear it; the retry offer —
and, while there, decide what `retryable()` should do for a run stopped by
`epoch` or `locked` (a retry can only meet the same refusal; a page that
offers it is offering nothing) and pin the decision. `FillStatus::Superseded`
has no deterministic test; give it one.

### 2. Refusals the doubles could not produce

- A refusal that arrives through the catalog (`Error::Index`) on the create
  and join paths: `library_files::write` opens the concrete `SqliteIndex`
  right after creating the file, and no double reaches it. Give the device
  layer a seam the tests can use — the smallest is a constructor that takes
  the opener — and pin that a catalog that will not open leaves
  `LibraryNotCreated { step: Index }` and no half-Library behind.
- The Drive path has no device-layer end-to-end test: `drive::grant` builds a
  `ReqwestTransport` itself and needs a real OAuth consent and a browser.
  Give `grant` / `transport` a signature that takes the transport, add a stub
  transport that answers the token endpoint and the app-folder listing, and
  cover `join_library` and `create_library` the way the S3 side is covered
  through its loopback stub.
- `refusal.test.ts` lacks the `UnreachablePlace` round trip; add it.
- The fetch route's 502 through `FetchError::Index` on the read side and the
  open side: `RefusingIndex` now exists; use it.

### 3. The gate does not see everything it should

- `make check` runs clippy with `--all-targets`, so the test targets are
  built and a warning that only the default build would raise is never seen;
  a test-only module once shipped in a release build through this gap. Add
  `cargo check` for the workspace (default features, default targets) to
  `make check`, with `-D warnings` through `RUSTFLAGS` or the manifest's
  lints table, whichever the workspace already uses.
- Write, in `Makefile`'s `check` comment or a short `docs/` note the
  Makefile points to, what `make check` does not run — `s3-store-it`,
  `e2e-it`, the Drive integration targets — and what covers each instead.
- `coffret-device`'s tests read the native root certificate store at run
  time and fail intermittently on macOS ("TrustStore configured to enable
  native roots but no valid root certificates parsed"); find the test that
  builds a real TLS client and give it a client that does not, or a
  certificate bundle from the tree.
- CI's e2e job downloads Chromium on every run; add a Playwright browser
  cache step keyed on the Playwright version.
- The contract between the server's DTOs and the TypeScript gateway is held
  by hand-written tests on each side. Add a golden-fixture contract test in
  the shape the format interop already uses: the server serialises each DTO
  and each refusal (every `error` / `reason` / `surfaced` combination,
  the `mapped` root case, each activity state) into fixtures, and a
  TypeScript test reads them through the real types and narrowing.

### 4. Tests a change left unwritten

- The retry give-up event (`gave_up`) has no direct test; the guarantee that
  it names no object is structural. Pin it against a captured log.
- A run that reaches commit says so once from the examine side; the early
  return is tested, the other half is not (`CapturedLogs` is thread-local,
  so this is a conformance-suite case).
- `init`'s `On Storage:` line: one stderr assertion in
  `a_library_is_created_mapped_and_listed`.
- `Progress::displaced` has neither a bound nor an expiry: a person who
  clicks through folders while Storage is down accumulates stopped
  activities that ride every poll until each is taken up. Decide what is
  forgotten and when (the oldest past a count, or anything older than the
  last successful poll), and pin it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] the web package runs its tests in `jsdom` and tests pin the
      `onUnmapped` argument for a click and a drop, `useActivity`'s
      `trouble`, the retry offer for `epoch` / `locked`, and
      `FillStatus::Superseded`
- [x] a catalog that will not open is pinned on the create and join paths;
      `join_library` and `create_library` run against a stub Drive
      transport; `refusal.test.ts` round-trips `UnreachablePlace`; the
      fetch route's `FetchError::Index` refusal is pinned read-side and
      open-side
- [x] `make check` runs a default-target `cargo check` with warnings denied,
      and states what it does not run
- [x] the native-root TLS read is gone from the device tests
- [x] a DTO / refusal golden-fixture contract test runs on both sides in
      `make check`
- [x] `gave_up`, the examine-side sentence, `init`'s `On Storage:` line and
      `Progress::displaced`'s bound are each pinned

### Manual / on-hardware (verified by a human before merge)

- [ ] the e2e job's Chromium cache hits on a second CI run

## Out of scope

- The MinIO conformance target's future
- Gateway timeout policy (no per-call ceiling on the Drive transport; the S3
  SDK's defaults implicit): its own change
- Any behaviour change beyond the bounds a test needs (`retryable()`,
  `Progress::displaced`), each decided and pinned here
