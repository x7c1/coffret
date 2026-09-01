---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -rq 'api/refresh' backend/crates/apps/coffret-server/src && grep -rq 'catch_up' backend/crates/apps/coffret-device/src && grep -rqi 'refresh' frontend/packages/apps/e2e/journeys"
assignee: null
branch: task/0902-0405-catch-up-the-catalog-in-the-explorer
created_at: 2026-09-01T19:05:14Z
updated_at: 2026-09-01T22:53:09Z
---

# feat(explorer): catch up the catalog on startup and on demand

## Overview

A device that joins a Library, or whose Library another device has since
written to, learns about the new state only through catch-up — replaying the
Journal records its Index has not seen (CK-9). Today the explorer has no way
to do that: `coffret-server` opens the Library and serves the Index as it
stands, so a freshly joined device's explorer looks empty, and a running
explorer never notices Containers another device committed. The only remedy
is running `coffret sync` / `coffret fetch` on the side, whose runs perform
catch-up internally (`coffret-usecase/src/commit/catch_up.rs`, called from
`fetch/run.rs`, `fetch/entry_run.rs`, and the sync flow) — but that entry
point is `pub(crate)` and drags the rest of those flows along.

Give the explorer its own catalog catch-up, in two places and no more:

- **On startup**: after opening the Library, the server catches the catalog
  up once, so a joined device's first explorer window shows the Library.
  A failure (Storage unreachable, expired token) must not kill the server —
  browsing what the Index already holds works offline by design; surface the
  failure the way background work already surfaces failures and let the
  manual refresh retry it.
- **On demand**: a refresh control in the browser UI, backed by a new
  `POST /api/refresh` route. No periodic polling — the explorer's existing
  discipline is zero requests while idle, and the reading use case only
  needs "the user asks to see what is new".

Concretely:

1. **`coffret-usecase`.** Expose a catalog-only catch-up flow: replay
   committed records into the Index and report what advanced (the
   generation range, and how many entries the catalog gained is enough for
   a status line). It is a thin public wrapper over the existing
   `commit::catch_up` + `read_committed`; it must not scan mapped folders,
   spool, fetch, or place anything locally. Follow the existing flow-module
   conventions (`fetch/run.rs` is the closest model).
2. **`coffret-device`.** `OpenLibrary::catch_up()` plus the one-unlock
   `run_*` wrapper, mirroring `run_fetch.rs` / `run_sync.rs`.
3. **`coffret-server`.** Run catch-up once at startup (non-fatal on error,
   reported like worker failures); add `POST /api/refresh` that runs the
   same flow with single-flight semantics (a second refresh while one runs
   joins or waits — never two concurrent replays), returning what changed
   so the UI can say "3 new entries" or "up to date". It shares the SQLite
   Index with the fill / sync workers, which WAL + busy_timeout already
   accommodates; reuse the worker plumbing where it fits rather than
   inventing a third pattern.
4. **Browser UI (`frontend/packages/apps/web`).** A refresh control (the
   toolbar / status bar is its natural home) wired through the
   `@coffret/api` gateway: on success, reload the folder tree and the open
   folder's listing and report the outcome in the status bar; on failure,
   show the existing retryable error presentation. No polling loops.
5. **E2E (`frontend/packages/apps/e2e` + `scripts/e2e-it.sh`).** Two
   additions to the MinIO E2E: (a) startup catch-up — a device joined by
   the API stage starts its server and the listing shows the Library
   without any CLI sync having run on that device; (b) manual refresh —
   while device B's server is up, device A commits a new file, and
   clicking refresh makes the new row appear. Screenshots land with the
   existing journeys.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret-device` exposes `catch_up` (the `grep -rq 'catch_up'` gate on
      the crate matches nothing today), and a test shows a second device's
      Index advancing to a record committed by the first device via the new
      flow alone — no fetch, no sync, no local file side effects.
- [x] `coffret-server` registers `POST /api/refresh` (the
      `grep -rq 'api/refresh'` gate matches nothing today) with router
      tests covering: a refresh that advances the Index and reports it, a
      refresh with nothing new reporting up-to-date, and a Storage failure
      surfacing as a retryable error without killing the server; plus a
      test that startup performs one catch-up and a startup with Storage
      unreachable still serves the existing Index.
- [x] The web app's refresh control calls the gateway and reloads tree +
      listing, covered by unit tests beside the existing fill / drop tests.
- [x] The E2E suite gains the startup catch-up and manual refresh journeys
      (the `grep -rqi 'refresh'` gate on the e2e package matches nothing
      today); they run under `make e2e-it` and the CI `e2e` job like the
      existing journeys.

## Out of scope

- Periodic polling or any always-on following of the remote head.
- Prefetching content after catch-up — the catalog advances; bytes still
  arrive through the existing open/background-fill path.
- The explorer freeze entry point and the catalog `renames` vocabulary.
- CLI changes — `sync` / `fetch` keep their internal catch-up as is.
