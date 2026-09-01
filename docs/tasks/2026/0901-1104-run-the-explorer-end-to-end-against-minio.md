---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && make e2e-it && find .tmp/e2e/screenshots -name '*.png' | grep -q . && grep -q 'e2e-it' .github/workflows/ci.yml && ! grep -rn --include=package.json --exclude-dir=node_modules '\"test\":.*playwright' frontend/packages"
assignee: null
branch: task/0901-1104-run-the-explorer-end-to-end-against-minio
created_at: 2026-09-01T11:04:00Z
updated_at: 2026-09-01T13:03:00Z
---

# test(e2e): drive the explorer's journeys end to end against MinIO

## Overview

Everything the explorer does — browse, read, backfill, accept a drop — is
proven today by router tests over in-memory adapters, and then by a person
at a browser walking each journey by hand. The by-hand half does not scale:
every explorer change re-asks a person to click through the same journeys,
and the layer between the two — the real `coffret-server` process, a real
SQLite Index on disk, real Storage, a `coffret sync` running beside the
server — is exactly the layer no in-memory test reaches. This task builds
that layer as one self-contained target, `make e2e-it`, in two stages, so
that after it a machine runs the journeys and a person's part shrinks to
looking at the screenshots the run saved and judging what only eyes can:
whether it looks and reads right.

The shape follows `scripts/s3-store-it.sh`: MinIO in Docker, started and
torn down by the script itself, nothing left running and no state carried
between runs. Unlike `drive-round-trip-it.sh`, state here is throwaway —
everything lives under `.tmp/e2e/` and is recreated per run; the one thing
a run leaves behind on purpose is `.tmp/e2e/screenshots/`, because the
pictures are the deliverable a person reviews. There is no pixel
comparison and no screenshot assertion beyond "the run saved them": a
journey passes or fails on what the page says and shows Playwright-wise,
and the pictures are for humans.

### Stage 1 — the API stage, as a script

`scripts/e2e-it.sh` (invoked by `make e2e-it`), in the house discipline of
`scripts/s3-store-it.sh` (own container name/port with env overrides, trap
teardown, bounded health polling — no bare sleeps) and of
`scripts/drive-round-trip-it.sh` (the CLI journey: `run_cli` piping a fixed
test Passphrase via `--passphrase-stdin`, reading summaries back out of the
transcript):

1. Build what the run needs once, up front: `coffret-cli`,
   `coffret-server`, `coffret-fixtures` (release), and the web app
   (`pnpm --filter @coffret/web build`).
2. Start MinIO, wait for health, create the bucket (the CLI checks that a
   bucket answers but does not create one — `docker exec` with the
   bundled `mc` serves).
3. Two devices out of two state directories under `.tmp/e2e/`, exactly the
   `drive-round-trip-it.sh` cast: `init --name main --s3 --bucket …
   --endpoint …` (path-style, against the container), `map` a `runs`
   prefix, generate a small fixture set (`coffret-fixtures`, on the order
   of a dozen photos and a few book pages — this is a journey, not a
   benchmark), `sync` to a committed head; then `recovery-code` →
   `join --name second` and one `sync --library second` to catch its Index
   up (a fresh join has no catch-up entry of its own in the explorer yet —
   a known, ledgered gap; the script syncs once instead).
4. Start `coffret-server --library second --passphrase-stdin --port
   <fixed, env-overridable>` (fixed rather than `--port 0`, because the
   browser stage restarts the server on the same port and the vite proxy
   is aimed once, at startup), and assert over HTTP what the one-shot
   verification of the serve-a-library work checked by hand, plus the
   upload path:
   - `/api/library` and `/api/folders` answer for the joined device;
   - the listing digs to the bottom: every folder `/api/list?path=`
     reaches from the root answers, and the rows under `runs/` are
     `remote` with the mapped bit set;
   - `/api/file?path=<one image>` is 200 with an image content type
     (a fetch from MinIO, placed and then served locally);
   - while the server is running, `coffret sync --library second` exits 0
     and the listing still answers afterwards — the WAL + busy_timeout
     cohabitation, against real files this time;
   - a multipart `POST /api/upload` into a mapped folder lands, the row
     shows as `uploading`, the armed sync commits, and a bounded poll sees
     the row become an ordinary Entry.

### Stage 2 — the browser stage, as Playwright journeys

A real Chromium driven by Playwright, against the same setup stage 1
built: the built web app served by `vite preview` (its `preview.proxy`
defaults to the dev `server.proxy`, which already reads `COFFRET_PORT` —
pass the e2e server port when starting it), the server serving `second`.
The suite lives in the frontend workspace as its own package beside
`apps/web`, wired like `@coffret/format`'s interop suite: behind a script
name the recursive `pnpm -r test` never runs (`test:e2e`), started only by
`scripts/e2e-it.sh`, with browsers installed idempotently by the script
(`playwright install chromium`; `--with-deps` on CI). `make check` must
stay exactly as Docker-free and browser-free as it is.

The server process belongs to the suite, not the script: the script hands
over the binary path, state directory, library name, port, and the MinIO
credentials env, and the suite starts the server once for all journeys —
because the outage journey has to kill and restart it, and a process the
script owned would be out of its reach. Four journeys, few on purpose:

1. **Browse and read** — open the explorer, walk the tree to the book
   folder, open a page in the reader, page forward and back with the
   arrow keys, Escape back to the list, reload and land where the URL
   hash says.
2. **Backfill** — open the album folder while its rows are still
   `remote`, open one image, and watch the rest go `fetching` → `present`
   with the progress line, until the activity is idle. (Ordering matters:
   this journey must run before anything else fetches the album.)
3. **Drop** — drop one small generated JPEG onto the open folder (a
   fabricated `DataTransfer` file drop; folder drops need
   `webkitGetAsEntry`, which a fabricated drop cannot carry — the
   traversal is router-tested and folder drops stay a by-hand check),
   see the `uploading` row at once, then the sync line, then the row
   settle into an ordinary Entry.
4. **Outage and recovery** — kill the server, see the error state the
   design puts on screen, restart the server on the same port, retry,
   and see the listing recover.

Each journey saves named screenshots at its checkpoints under
`.tmp/e2e/screenshots/<journey>/` (wiped at run start). On failure, keep
whatever Playwright's own trace/screenshot-on-failure leaves as well.

### One target, and CI

`make e2e-it` runs both stages and is the whole interface, documented the
way `s3-store-it`'s Makefile comment block documents that target: what it
covers, what it needs (Docker; downloads a browser on first run), where
the screenshots and logs land. `.github/workflows/ci.yml` gains an `e2e`
job in the shape of the `interop` job (both toolchains, both caches) that
runs `make e2e-it` and uploads `.tmp/e2e/screenshots/` as an artifact —
`if: always()`, because the pictures of a failing run are the ones most
worth looking at.

### What already exists, for the work

- `scripts/s3-store-it.sh` — the MinIO container discipline to reuse.
- `scripts/drive-round-trip-it.sh` — the CLI journey in bash: `run_cli`,
  Passphrase via stdin, reading the Recovery Code and summaries back.
- `backend/crates/apps/coffret-server/src/main.rs` — `--library`,
  `--passphrase-stdin`, `--port`; prints the bound address on stderr.
- `backend/crates/apps/coffret-cli/src/init.rs` — `--s3 --bucket
  --endpoint` (+ path-style addressing flag) already exist; likewise
  `join`, `map`, `sync`, `fetch`, `recovery-code` in the sibling modules.
- `frontend/packages/apps/web/vite.config.ts` — the `COFFRET_PORT` proxy.
- `frontend/packages/gateway/api/src/` — the wire vocabulary the journeys
  assert against (row states, activity, refusals).
- The `deps` target's gates — the e2e package must not trip the raw
  `fetch(` gate (it is scoped to `apps/web/src`) and must not grow into a
  second reading of the wire contract: journeys assert through the UI;
  API-shaped assertions belong to stage 1.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make e2e-it` is self-contained and green: it builds what it needs,
      starts and tears down its own MinIO (trap on exit), recreates
      `.tmp/e2e/` per run, and fails the target when any stage fails.
- [x] The API stage proves, against real processes on MinIO: the listing
      answers to the bottom with `remote` mapped rows after a join plus
      catch-up sync; `/api/file` serves an image 200; `coffret sync`
      exits 0 while the server is running and the listing answers
      afterwards; a multipart upload lands, is listed `uploading`, and
      becomes an ordinary Entry after the armed sync commits.
- [x] The browser stage runs the four journeys (browse-and-read,
      backfill, drop, outage-and-recovery) in real Chromium and saves
      checkpoint screenshots under `.tmp/e2e/screenshots/<journey>/`
      (the check command asserts at least one PNG exists after the run).
- [x] The Playwright suite is not reachable from `pnpm -r test` (no
      `test` script invokes playwright — enforced by the grep gate in the
      check command) and `make check` itself needs neither Docker nor a
      browser.
- [x] `.github/workflows/ci.yml` has an `e2e` job that runs `make e2e-it`
      and uploads the screenshots directory as an artifact with
      `if: always()` (the check command greps ci.yml for the target).

### Manual / on-hardware (verified by a human before merge)

- [x] The screenshots a green run saved read right to a human at each
      checkpoint: the filer list with its chips, a book page in the
      reader, the backfill progress, the `uploading` row, and the outage
      notice with its recovery — layout, wording, and states as designed.

## Out of scope

- A real-Drive E2E variant (a manual target beside `drive-round-trip-it`)
  — a later task; this one is the unattended MinIO path.
- Pixel comparison, visual regression, or any assertion on image bytes —
  screenshots are saved for humans, not compared by machines.
- Folder drops in the browser stage (a fabricated drop cannot carry
  directory entries; the traversal is router-tested).
- Serving the built web app from `coffret-server` — the journeys go
  through the vite proxy, as `make web` does today.
- A catch-up entry point in the explorer for a freshly joined device (the
  ledgered gap; the script's one `sync` on the joined device stands in).
- New explorer features or route changes of any kind; CI wall-clock work
  beyond reusing the existing cache steps.
