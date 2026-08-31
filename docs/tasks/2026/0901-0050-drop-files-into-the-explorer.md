---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, user-experience, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/0901-0050-drop-files-into-the-explorer
created_at: 2026-09-01T00:50:00Z
updated_at: 2026-09-01T03:35:00Z
---

# feat(explorer): drop files into the explorer to add them to the Library

## Overview

The explorer can browse, read, and backfill a Library, but nothing goes
*in* through it: adding a file still means placing it into the mapped
folder by hand and running `coffret sync` in a terminal. The decided
design closes that gap with drag and drop: files or folders dropped onto
the folder the explorer has open are written into the device's mapped
folder and the existing sync pipeline takes it from there — encryption,
upload, commit, and the books all stay exactly where they are. The drop
means one thing, "add"; spool → put → commit is one automatic motion, and
only its visibility is staged.

### The receiver, server-side

- **`POST /api/upload?folder=<path>`**, multipart. Each part is one file;
  its filename field carries the path **relative to the drop target** (a
  plain file drop sends `photo.jpg`; a folder drop sends
  `holiday/day1/photo.jpg`), so one route serves both shapes. The Entry
  Path a part lands at is `<folder>/<relative>`, normalized and shape-
  checked like every other incoming path (EP-1, EP-2); a part whose
  relative path fails the shape rules is refused by name.
- **Refusals are decided before any byte lands**, per part, against the
  Index:
  - the drop target (or the part's subpath) reaches no mapping — the same
    "not on this device" the listing already shows (EP-9); the whole drop
    is refused when the target folder is unmapped;
  - a current Entry already stands at the path and its Container is a
    **Pack** — refused for now (read-modify-replace of a Pack, PK-10..12,
    is not built; writing the file locally would strand it in a state the
    pipeline cannot propagate, which is exactly the limbo the design
    forbids putting on screen);
  - a current Entry stands there in a **one-file Container** — accepted:
    the sync pipeline already treats a changed mapped file as a normal
    replacement (CP-14).
  The response lists, per part: written, or refused with the reason —
  the same JSON refusal vocabulary the other routes use.
- **Writes are whole or absent.** Each accepted part streams to a
  temporary name inside the destination directory and is renamed into
  place, so a half-received upload never sits where the scan would read
  it as a file the person put there. The temporary name uses the reserved
  scratch prefix the scan already passes over (EP-11 rule 6 reserves the
  prefix for fetch's own temporaries; serving a second writer from the
  same reservation needs a sentence added to that rule's doc comment in
  the code — the register change itself is docs-pass work). The mapped
  local path for a new Entry is derived by the same EP-9 translation the
  fetch side uses — one rule, one implementation, as `coffret-device`
  promises.
- **Sync runs by itself afterwards.** When a drop lands at least one
  file, the server arms its background sync — a second single worker
  beside the fill's, same watch-channel shape, same drop-guard
  discipline: at most one sync at a time; a drop during a running sync
  queues exactly one follow-up run (two drops queue one run — the scan
  picks up both). The CLI's `sync` semantics are untouched; the server
  simply runs the same flow the person would have typed.
- `GET /api/activity` grows a `sync` side next to `fill`: running or not,
  and — when the last run stopped on a Storage error — the same reported
  refusal + retry shape the fill has (`POST /api/sync` re-arms it; like
  `POST /api/fill` it exists to recover, not as a "sync button").
  Findings a sync surfaces (a conflict, a vanished file) are reported in
  the activity like the fill's declined list, so the person who dropped
  is told without opening a terminal.
- Logging as always: counts, sizes, durations, outcome kinds; no Entry
  Paths, no local paths.

### The rows, browser-side

- **A dropped file appears in the listing immediately.** The listing
  route merges a third row source: local files under the open folder's
  mapped root that no current Entry stands at (the Index's
  `present_without_entry`, narrowed to the folder), shown with an
  `uploading` chip — the design's "not yet in the Library" state. They
  are real files, so they open in the reader like any `present` row.
  When the sync commits, the row becomes an ordinary Entry row on the
  next listing refresh (the activity's sync transition drives that
  refresh, like the fill's counts do).
- **The drop itself**: the file list is the drop zone for the folder it
  shows. Folder drops traverse the dropped directory tree
  (`webkitGetAsEntry`); an unmapped folder never accepts a drop — the
  same banner state that disables its rows marks it — and a refused part
  (Pack-resident name, bad name) surfaces in the notice area with the
  server's message, while accepted parts proceed. Upload progress is the
  status bar's transient line ("adding <n> files to <folder>…"), then the
  sync's own line takes over.
- The gateway package gains the upload call (multipart `FormData`) and
  the extended activity types; chip derivation (`uploading` beside
  `fetching`/`failed`) stays in the pure module with its tests, as does
  the drop-traversal → relative-path list function (pure, testable
  without DOM).

### Tests

Router-level, in-memory: a two-part upload lands both files and the
listing shows them as `uploading` rows before sync and Entry rows after
the armed sync commits; an upload into an unmapped folder is refused
whole with the established reason; a part whose name collides with a
Pack-resident Entry is refused by name while its sibling lands; a part
with a `..` segment is refused; a drop during a running sync queues
exactly one follow-up; a sync stopped by Storage reports through the
activity and `POST /api/sync` finishes after the store recovers; the
scratch-named temporary of an interrupted upload is not listed as an
`uploading` row. Frontend: pure tests for the traversal → relative-path
list, the extended chip derivation, and the polling seam's new condition.
The deterministic-job seam built for the fill is reused; no sleeps.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes, including the new upload/sync-activity router
      tests and the frontend traversal / chip-derivation tests.
- [x] The `deps` target's rules still hold unchanged (`coffret-server`
      reaches the domain only through `coffret-device`; no raw `fetch(`
      in the web app outside the gateway package).

### Manual / on-hardware (verified by a human before merge)

- [ ] On real Drive (the round-trip state, any mapped Library): dropping
      a few images onto an open run folder shows them at once with
      `uploading` chips, the status line reports the add and then the
      sync, the chips settle to ordinary rows, and reloading the browser
      shows them as normal `present` Entries; the dropped images open in
      the reader throughout.
- [ ] Dropping a folder (with a nested subfolder) adds its files under
      matching subpaths, and a second device (`second`) fetches the new
      Entries with `coffret fetch`.

## Out of scope

- Lifting the Pack-resident same-name refusal (update propagation into
  Packs, PK-10..12 — the ledgered next priority after this milestone).
- Deletion through the explorer, and any delete/evict UI.
- Upload resume, parallel multipart streams, or progress percentages per
  file.
- A "sync now" button (`POST /api/sync` recovers; it does not add a
  manual operation to the design).
- Setup (init/join/map) through the explorer.
