---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "turn" docs/concepts/library/README.md'
assignee: null
branch: task/1014-0338-say-which-run-holds-the-turn-and-keep-the-status-bar-current
created_at: 2026-10-13T18:38:00Z
updated_at: 2026-10-14T05:30:00Z
---

# feat(server,explorer): say which run holds the turn, and keep the status bar on the latest run

## Overview

Four findings from using the explorer while sync, freeze and delete take
turns on the pending rows (the server's one fair mutex, `PendingRows` in
`coffret-server/src/pending_rows.rs`). They share one cause: the server does
not say who holds the turn, so the explorer guesses, and the status bar ranks
its one line by a fixed order of flows rather than by what is happening now.

### 1. The server says who holds the turn and who waits

`GET /api/work` (`routes/work/work_dto.rs`) reports each flow with a `step`
and no word about the turn. `DeleteDto.waiting` and `FreezeDto.waiting` count
what is queued *behind* a run of the same flow, which is a different thing.
The explorer (`turns.ts` `waitsFor`, `:47-60`) infers that a run with
`step: null` is waiting and guesses which other run it waits for, and in the
window between taking the turn and the first step the guess reads backwards:
two runs can each claim to wait for the other, which is the "waiting for the
deletion" line that lingered after the deletion had ended.

Make it explicit:

- `WorkDto` gets `turn: Option<"sync" | "freeze" | "delete">`: the flow whose
  run holds the pending rows now, `null` when nobody does. `PendingRows`
  learns who holds it (set when a run takes it, cleared when it is given
  back), and `PendingRows::waiting()` stays as it is.
- Each of `SyncDto`, `FreezeDto`, `DeleteDto` gets `waits: bool`: true from
  the moment the run is on record until it holds the turn. A run with
  `waits: false` and `step: null` is one that has the turn and has not
  reported a step yet.
- The explorer's `waitsFor` reads `waits` and `turn` and no longer infers
  from `step`; its clause says "waiting for the <turn holder>" and nothing
  when `turn` is null. Update `routes/work/contract.rs` shapes and the
  `turns.test.ts` cases, and add a server route test
  (`tests/routes/turns.rs` style) that reads `turn` and `waits` while a sync
  waits behind a deletion.
- **The concept and the spec say what a turn is.** Nothing in
  `docs/concepts` or `docs/spec` describes it today. Add to the Library
  concept (`docs/concepts/library/README.md`) a short paragraph: within one
  server, sync, freeze and delete take turns on the pending rows in the order
  they were armed, and a waiting run is reported as waiting for the holder;
  across processes there is no queue, and the second is refused at once
  (the refusal kind `conflict`, which `api_error/server.rs:125-155`
  documents for the CLI's sync / freeze beside a server). Keep it to a few
  sentences and cite OC-2 and LA-12 as they stand; do not edit the spec's
  LA rules in this task — a change to them is decided elsewhere. The gate in
  `check_command` greps `turn` in the Library concept.

### 2. The status bar shows the latest thing, not the highest-ranked flow

`StatusBar.tsx:203-231` shows one line, the first non-null of: the drop's
upload line, the freeze line, the sync line, the fill line, stopped runs,
discarded runs. A finished freeze keeps its `packed …` line (`fill.ts`
`freezeLine:371-393`), a finished sync has no line at all (`syncLine:328-342`
returns null when there are no findings), and the deletion is never in the
bar. So after a Pack is made, every later one-by-one add or delete leaves the
old `packed …` in place, and a run waiting its turn can hide the one that is
moving.

Rank by state instead of by flow:

1. the drop's own upload line (a thing this tab is doing);
2. the run that holds the turn (`turn`), with its step;
3. a run that waits (`waits: true`), saying for whom;
4. the most recently finished run's result line, whichever flow it was —
   `packed …`, `added N files to X` (from `SyncDto.added`, which the explorer
   does not read today; add a sentence for it in `fill.ts`), or `deleted …`
   (the deletion's running and result lines move into the bar from above the
   list, `App.tsx:990-1002`; the `deletedLine` notice in the list stays, it
   is about this tab's own action);
5. the fill line, stopped, discarded, as today.

"Most recently finished" needs an order between flows: use the run numbers'
publication order the explorer already sees (a finished run replaces the
previous result when its `run` is newer in the poll that reported it), or a
timestamp if the DTOs have one; keep whatever rule you pick in one function
with a test. The dismiss button (`whoseLine`, `:455-500`) follows the same
ranking. Update `StatusBar.test.tsx` and `fill.test.ts` accordingly.

### 3. A new folder from a one-by-one add reaches the tree

A sync's commit can create a folder, but only the listing is reloaded when
the sync's status changes (`App.tsx:436-445`); the tree reloads on a freeze
(`:446-464`) and on refresh. Reload the folders on a sync status change too,
with the same shape as the freeze effect.

### 4. Every finding's `said` line starts with its kind

`coffret-device/src/finding.rs:357-367` writes a `Surfaced` finding as
`surfaced <path>: <reason>` except when the reason is `MovedToTrash`, which
drops the kind word because "surfaced" is not what was done. Every other
variant starts with a kind word (`unavailable root`, `key-lost container`,
…), and the CLI's `said` field (`coffret-cli/src/answer/found.rs:191-193`)
is read by scripts that split on it. Give the trash case a kind word of its
own — `moved <path>: <reason>`, since *move* is the Library concept's verb
for a departed file going to the desktop's trash and *trash* is reserved for
a Storage Object — and keep the reason text; add the Display assertion that
is missing for it beside `findings.rs:373`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A server route test: while a deletion holds the turn and a sync is
      armed behind it, `GET /api/work` answers `turn: "delete"`,
      `sync.waits: true`, `delete.waits: false`; once the deletion is done and
      the sync holds the turn, `turn: "sync"` and `sync.waits: false`; with
      nothing running, `turn: null`
- [x] `turns.test.ts`: `waitsFor` reads `waits` and `turn` from the work
      answer and never infers from `step`; a run with `waits: false` and
      `step: null` is not reported as waiting
- [x] `StatusBar.test.tsx`: after a `packed …` result, a later finished
      sync shows `added N files to X` and a later finished deletion shows its
      result; a run holding the turn outranks a waiting one; a waiting run
      says whom it waits for
- [x] A test of the reload effect, or of the function it calls, showing that
      a sync status change requests the folders as well as the listing
- [x] `findings.rs`: the `MovedToTrash` surfaced finding displays as
      `moved <path>: <reason>`, and the CLI JSON fixture in
      `coffret-cli/src/answer/tests.rs` covers it
- [x] `make check` passes; the Library concept mentions the turn (gate in
      `check_command`)

### Before merge (verified outside the check command)

- [ ] On the agent's Library (`.tmp/agent/up.sh 1`, headless Chromium): drop
      a book as a Pack, then add one file one by one, then delete it; after
      each run the status bar shows that run's result; while a deletion runs,
      a dropped book's line says it waits for the deletion and, once the
      deletion is done, the waiting line is gone within one poll; a folder
      made by a one-by-one add appears in the tree without pressing
      "look for what is new"
