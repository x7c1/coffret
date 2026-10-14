---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design, rust-module-structure, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "no row" docs/spec/pack-construction/README.md'
assignee: null
branch: task/1014-0340-say-what-a-parcel-read-left-behind-and-show-what-is-kept
created_at: 2026-10-13T18:40:00Z
updated_at: 2026-10-14T04:20:31Z
---

# fix(fetch): say what a parcel read left behind, sweep parcel files no row names, and show what is kept

## Overview

Four findings from the parcel-based fetch (PRs #271, #272): what happens in
the part of a read that continues after the Entry was answered, and what the
person can see of the parcels kept on the device.

### 1. A panic after the answer is logged

`EntryFetches::fetch` (`coffret-device/src/entry_fetches/mod.rs:126-185`)
spawns the read with `tokio::spawn` (`:142`) and, once the Entry has been
answered through the oneshot, drops the `JoinHandle` (`:176-184` await it
only when the answer was never sent). A panic in the rest of the read then
reaches nothing but the default panic hook's stderr line; the log under
`COFFRET_LOG_DIR`, which is what a person reads to tell a failure apart,
says nothing. The fill, sync, freeze and delete workers have a `Leaving`
guard that puts their state back on a panic (`coffret-server/src/fill/worker.rs:9-41`
and the three siblings), and it does not log either.

Observe the task's end: after the answer is sent, keep the handle and await
it from a small watcher task (or `catch_unwind` the body), and on a panic
emit `error!(operation = "fetch_entry", …)` saying the rest of a parcel read
panicked after its Entry was answered, with the Entry Path redacted the way
other fetch events redact it (EL-5). Give the four `Leaving` guards a `warn!`
of the same shape (`operation = "fill"` and so on) when `thread::panicking()`
is true, so a worker's panic is in the log beside the state it put back.

### 2. A read that fails after the answer still says which parcels it did not hold

`finished_after_answer` (`entry_fetches/mod.rs:284-302`) hands the
`UnheldParcel` findings to the fill on `Ok` and only warns on `Err`. The list
is built in `Herald` (`coffret-usecase/src/fetch/parcel_read/herald.rs`) and
dropped with it on every error path of `read_entry`
(`parcel_read/read_entry.rs` ~`:114-197`); only the success path reaches
`stroke.unheld = herald.into_unheld()`. PK-21 says a parcel that did not
arrive whole is said and re-read whole: the saying must not depend on the
rest of the run succeeding.

Let the unheld list survive the error: have the caller own the sink the
read reports into (pass the `Herald`, or a `&mut Vec<UnheldParcel>`, in from
`EntryFetches::fetch`, so it is still there when `read_entry` returns `Err`),
or make the error carry the list. Then `finished_after_answer` hands over
what was found in both branches and still warns about the failure. Extend
`the_rest_of_a_stroke_that_fails_after_the_answer_is_logged_and_let_go`
(`coffret-server/tests/routes/file.rs:680`) to assert the unheld finding
reaches the work answer.

### 3. A parcel file no row names is discarded at open

`Source::fetch` (`parcel_read/source.rs:108-155`) writes the held-parcel row
first and the file second (OC-2's order). A catch-up's `let_go_departed`
(`parcel_read/let_go.rs:37-66`) takes no gate a fetch holds, so in a narrow
window it can read the fresh row, find no file yet (`discard` tolerates a
missing file), delete the row, and the fetch then writes a file nothing
names. `ParcelFiles` (`coffret-usecase/src/parcel_files.rs:41-69`) has no
list operation, and its doc at `:28-29` says nothing lists the directory, so
such a file stays forever, up to S each.

Add `ParcelFiles::list` (the file names in the parcel directory) and a sweep
at Library open, after the startup catch-up's `let_go_parcels`: every file in
the directory whose `{container_id}-{index}.parcel` name (`kept_parcels.rs:56-58`)
is not among `held_parcels()` is discarded, with one `info!` per sweep saying
how many were. Implement it in the usecase beside `let_go_parcels` with a
conformance case (a file planted with no row is gone after open; a held one
stays), and the in-memory and real `ParcelFiles` both grow the list method.
Update the `parcel_files.rs` doc and the Index concept's held-parcel
paragraph (`docs/concepts/index/README.md:32-40`: the Index still names the
files; the sweep only removes what it does not name), and add one sentence
to PK-21's "a kept parcel is device state" sub-point: a parcel file no row
names is discarded at the next open (the gate in `check_command` greps
`no row` in the Pack spec).

### 4. The kept parcels are counted in the work answer and shown in the status bar

Nothing tells the person that `<library dir>/parcels/` exists or how big it
is; a reader who opens the first page of many books leaves one or two
parcels per folder behind, which is gigabytes over time. The Index has
`held_parcels()` with each row's `file_path`; sizes come from a `stat` of
each (dozens of files, cheap) through the local-fs gateway, never by walking
the directory.

Add to `WorkDto` (`coffret-server/src/routes/work/work_dto.rs`) a
`kept_parcels: { count, bytes }` under the catalog or beside it, computed
from the rows on each poll, and update `routes/work/contract.rs`. In the
explorer's status bar (`StatusBar.tsx`), next to the Library's name and
provider, show `N parcels kept, X MB` when `count > 0` and nothing when it
is zero; the number formatter the bar already uses for bytes applies.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A server route test: a parcel read that panics after the Entry was
      answered leaves an `error` event with `operation = "fetch_entry"` in
      the captured logs, and the next fetch of the same Entry is served
- [x] Unit tests on the four `Leaving` guards (beside
      `a_worker_that_panics_is_put_back`): a panic leaves a `warn` event
      naming the worker's operation
- [x] `the_rest_of_a_stroke_that_fails_after_the_answer_is_logged_and_let_go`
      also asserts the unheld parcel met before the failure appears in the
      work answer's fill findings
- [x] A fetch conformance case: a `.parcel` file planted in the parcel
      directory with no row is discarded when the Library is opened, a held
      parcel's file stays, and the sweep logs the count
- [x] A server route test: `GET /api/work` reports `kept_parcels.count` and
      `bytes` equal to the held parcels after a fetch, and zero before
- [x] `StatusBar.test.tsx`: the bar shows `N parcels kept, X MB` when the
      work answer reports a count and nothing when it is zero
- [x] `make check` passes; PK-21 mentions the parcel file no row names
      (gate in `check_command`)

### Before merge (verified outside the check command)

- [x] On the agent's Library (`.tmp/agent/up.sh 2` on the second device,
      headless Chromium): opening one page of `lib/book-000` leaves one
      parcel under `~/.local/state/coffret-agent-2/libraries/agent/parcels/`,
      the status bar says `1 parcel kept` with its size, and a stray
      `deadbeef-0.parcel` planted there is gone after `down.sh 2` and
      `up.sh 2`, with the sweep's line in the server log
