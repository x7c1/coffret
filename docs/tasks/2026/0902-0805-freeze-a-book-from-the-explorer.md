---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -rq 'api/freeze' backend/crates/apps/coffret-server/src && grep -rqi 'freeze' frontend/packages/apps/web/src && grep -rqi 'freeze' frontend/packages/apps/e2e/journeys"
assignee: null
branch: task/0902-0805-freeze-a-book-from-the-explorer
created_at: 2026-09-01T23:05:37Z
updated_at: 2026-09-02T02:40:17Z
---

# feat(explorer): freeze a book from the explorer

## Overview

The intended daily use of the explorer is bringing in scanned books one at a
time: a book is one folder of page images, and packing it with `freeze`
uploads it as a few Packs instead of hundreds of one-file Containers — which
is also what keeps the provider API call count low. The machinery exists end
to end (`freeze_folder` with its PK-17 folder scope, `OpenLibrary::freeze`,
`coffret freeze --under <path>`), but the only entry point is the CLI. Give
the explorer one:

- **Create a folder in the browser.** The UI gains a "new folder" action
  under a chosen parent. Folders are Entry Path prefixes, not first-class
  objects, so a newly created folder is a UI-side temporary until its first
  Entry commits — no server or catalog change is needed to create one.
- **Drop a book into it, and it freezes.** Dropping files into a folder the
  user just created in the browser is a book import: the parts are received
  like the existing upload route receives them (streamed, scratch-name
  whole-or-absent writes into the mapped location, per-part rejections
  before any byte lands — an unmapped target rejects the whole drop, an
  invalid path rejects that part), **but the drop does not arm the sync
  worker**. Instead, once the drop's parts have landed, a freeze runs over
  that folder (`OpenLibrary::freeze` with the folder as the PK-17 prefix),
  so the pages upload exactly once, as Packs. Dropping into an existing
  folder keeps today's sync-based D&D semantics unchanged.
- **One book at a time.** The freeze runs on a dedicated single-flight
  worker in the same mold as the fill and sync workers: observable through
  `GET /api/activity`, never two freezes at once, and a `POST /api/freeze`
  (folder path) exists to retry a failed or interrupted freeze — mirroring
  how `POST /api/fill` is the recovery entry, not the normal path. While a
  freeze is running the UI does not offer a second one.
- **What the user sees.** The dropped rows appear immediately as
  not-yet-committed (openable, as with today's uploads), the folder shows a
  freezing state while the worker runs, the status bar carries progress the
  way sync progress is carried, and on commit the rows become ordinary
  `present` entries. A failure surfaces as the existing retryable error
  presentation, with `POST /api/freeze` behind the retry.

Concretely:

1. **`coffret-server`.** A freeze worker beside the fill and sync workers
   (single-flight, drop-guarded, activity-reported); `POST /api/freeze`
   accepting a folder path (reject unmapped, reject while another freeze
   runs by joining/queueing exactly like the sibling workers do); the
   book-drop receiving path — reuse the existing multipart receiving and
   `IncomingFile` validation, differing only in that it arms the freeze
   worker for the target folder instead of the sync worker. How the route
   distinguishes a book drop from a plain drop is an API-shape decision:
   prefer an explicit request parameter set by the UI (the UI knows the
   drop landed in a browser-created folder) over server-side guessing.
2. **Shared default.** The default Pack target size currently lives in the
   CLI (`coffret-cli/src/freeze.rs`, one gibibyte). Hoist it into
   `coffret-device` so the CLI and the server share one number, with the
   CLI flag still able to override it.
3. **Browser UI (`frontend/packages/apps/web`).** The "new folder" action
   (client-side temporary until its first Entry commits — it must survive
   listing reloads while its drop/freeze is in flight and disappear
   gracefully if abandoned); drop-into-new-folder wiring to the book-drop
   request; the freezing folder state, status-bar progress, and the
   retryable failure presentation; disable offering a second freeze while
   one runs. Gateway additions in `@coffret/api`.
4. **E2E (`frontend/packages/apps/e2e`).** A journey: create a folder in
   the browser, drop page images into it (file drops — folder drops cannot
   be faked in Playwright), watch the freeze run and the rows become
   `present`, and confirm via the API stage that the committed Containers
   for that folder are Packs, not one-file Containers; then a second
   device's refresh shows the book. Screenshots land with the existing
   journeys.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret-server` registers `POST /api/freeze` and the book-drop
      receiving path (the `grep -rq 'api/freeze'` gate matches nothing
      today), with router tests covering: a book drop that lands parts and
      freezes them into Packs (the committed Containers for the folder are
      pack-kind), an unmapped target rejected before any byte, a same-name
      part rejected per the existing per-part rules, a second freeze while
      one runs not producing two concurrent runs, and a freeze failure
      surfacing as a retryable error with `POST /api/freeze` succeeding
      afterwards.
- [x] The default Pack target size is defined once in `coffret-device` and
      used by both the CLI and the server (the CLI flag still overrides).
- [x] The web app covers the new-folder lifecycle and the freeze-drop wiring
      with unit tests beside the existing drop/fill/refresh tests (the
      `grep -rqi 'freeze'` gate on the web package matches nothing today).
- [x] The E2E suite gains the book-freeze journey (the `grep -rqi 'freeze'`
      gate on the e2e package matches nothing today), running under
      `make e2e-it` and the CI `e2e` job, including the Pack-kind assertion
      and the second device seeing the book after refresh.

## Out of scope

- Updating or deleting entries inside existing Packs (PK-9 through PK-12),
  and lifting the same-name-onto-Pack drop rejection.
- Catalog-level renames and any EP-rule rewording.
- Thumbnails, `derive`, and changing the plain-drop (sync-based) semantics
  of dropping into existing folders.
- CLI changes beyond sharing the default Pack target size constant.
- Freeze of folders that already hold committed loose entries from earlier
  syncs — the CLI remains the entry point for that until a later task.
