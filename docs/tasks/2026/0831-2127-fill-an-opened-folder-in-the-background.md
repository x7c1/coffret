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
branch: task/0831-2127-fill-an-opened-folder-in-the-background
created_at: 2026-08-31T21:27:00Z
updated_at: 2026-08-31T23:59:00Z
---

# feat(explorer): fill an opened folder in the background

## Overview

Opening a `remote` image today fetches exactly that image: the range read
brings one Entry over, the reader shows it, and every other file in the
folder stays `remote` until someone clicks it. For a book that is the
wrong shape — the person who opened page one is going to read page two —
and the design has always said the rest of an opened Pack is brought over
"by the same mechanism as the prefetch", in the background. The reader's
own prefetch (radius 3) papers over this for adjacent pages, but it stops
when the reader closes, re-fetches nothing for the list view, and leaves
a half-`remote` folder behind.

This change adds that background fill: when serving `/api/file` makes the
server fetch a `remote` Entry, the server starts filling the rest of that
Entry's folder on its own, and the browser shows it happening — the
design's `fetching` / `failed` states, which so far existed only as the
browser's own request lifecycle, become visible for work the server does
unasked.

### The fill, server-side

- **Trigger**: implicit, per the design's "fetching is implicit" rule —
  there is no download button. When a `GET /api/file` request had to
  fetch (the verdict was a placement, not `AlreadyPresent`), the folder
  holding that Entry becomes the fill target. Re-triggering for the same
  folder while its fill runs is a no-op; a fetch landing in a different
  folder makes that folder the *next* target (latest wins — the person
  moved on, so the fill follows them; the interrupted folder is not
  resumed automatically, clicking into it again re-arms it).
- **One fill at a time**, run by a single background task owned by the
  server (the CLI is untouched — a one-shot process has nobody left to
  fill for). The job walks the target folder's listing and brings each
  `remote` file over **through the same per-Entry single-flight fetch the
  routes use** — that is the point of the existing `EntryFetches` gate:
  the reader's prefetch, a second click, and the fill can all ask for the
  same Entry and it is fetched once, each placement writing its own
  scratch file and renaming it into place (EP-11). Per-Entry range reads
  instead
  of one whole-Container read is a deliberate first shape — it reuses the
  placement discipline unchanged; coalescing adjacent Entries of one Pack
  into one read is a later optimization and gets a code comment saying so
  (PK-16 makes it legitimate whenever we want it).
- **A declined Entry does not stop the fill.** `Surfaced` verdicts and
  locked Containers are the fill's findings, not its failure: record them
  and keep going, exactly as the CLI's `fetch` does. A Storage error
  (unreachable, expired grant) *does* stop the fill — every further Entry
  would fail the same way — and is reported once.
- **Progress is observable**: `GET /api/activity` returns the current
  fill — its folder, how many files are done out of how many it set out
  to bring over, the Entries it declined (path + reason), and whether it
  stopped on a Storage error — or that nothing is running. This is the
  server's own state, not the Index's: device state about work in flight,
  gone on restart, never uploaded.
- **Retry is explicit and folder-scoped**: `POST /api/fill?path=<folder>`
  re-arms the fill for that folder (the UI's retry after a Storage error;
  also usable when a fill was superseded). It is not a download button —
  it exists to recover, and the UI offers it only from the failed state.
- Logging follows the crate's rule: counts, durations, outcome kinds; no
  Entry Paths, no local paths.

### The chips, browser-side

- While the reader is open or `/api/activity` says a fill is live, the
  app polls activity (a small interval; stop polling when idle — an idle
  explorer must issue zero requests). Rows in the filling folder that the
  listing still calls `remote` show the `fetching` chip; when the
  activity's counts advance, refresh the folder listing so chips flip to
  `present` as files land (the listing stays the single source for
  `present` / `remote` — activity only adds the transient overlay).
- A fill that stopped on a Storage error shows a `failed` chip on the
  affected rows and one line in the status bar with a retry that calls
  `POST /api/fill`. Declined Entries (surfaced / locked) are not
  `failed` — they show the message the file route would give when opened,
  and the fill's activity lists them so the UI can mark them without a
  click.
- The status bar's transient line grows to cover the fill ("bringing
  over <done>/<total> in <folder>…"), replacing the per-file line when
  both would show.
- The chip derivation (listing + activity → per-row state) is a pure
  function with vitest cases beside the existing ones; polling start/stop
  is also factored to a testable seam. No DOM test infrastructure.

### Tests

Router-level (in-memory store, no sockets): a `GET /api/file` that fetched
starts a fill and `/api/activity` reports it; the fill completes and every
file in the folder is `present` in the listing; a store that fails after N
objects leaves a Storage-stopped activity with the untouched remainder,
and `POST /api/fill` after the store recovers finishes the job; a fill
target superseded by a fetch in another folder; a surfaced Entry is listed
as declined and the fill still completes; the same Entry asked for by the
fill and a concurrent `/api/file` is placed once (range-read count). The
job must be drivable deterministically in tests — no sleeps; give the
tests a way to run or await the job (the implementer picks the seam).
Device layer: no new primitives expected (the fill composes `list` +
`fetch_entry`); if one is added it gets its own unit tests.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes, including the new fill/activity router tests,
      the range-read single-flight case, and the frontend chip-derivation
      and polling-seam tests.
- [x] The `deps` target's rules still hold unchanged (`coffret-server`
      reaches the domain only through `coffret-device`; no raw `fetch(`
      in the web app outside the gateway package).

### Manual / on-hardware (verified by a human before merge)

- [ ] On real Drive, on a device where the run folders are `remote`
      (join a fresh device-side name with the Recovery Code, map `runs`):
      opening a run folder and clicking the first image shows it within a
      few seconds, and **without further clicks** the folder's other rows
      flip to `present` as the fill proceeds, with the status bar
      counting progress.
- [ ] Clicking another image while the fill runs opens it (instantly if
      already filled) and the fill still finishes with no `failed` rows.

## Out of scope

- Coalescing a Pack's adjacent Entries into one range read (noted in
  code; PK-16 covers the semantics).
- More than one concurrent fill, fill persistence across server restarts,
  or resuming a superseded fill unprompted.
- Server-driven push (WebSocket/SSE) — polling is enough at this scale.
- The drag-and-drop upload receiver (the next change).
- Prefetch-radius retuning for the E2EE path (measured later, on the
  benchmark scenarios).
