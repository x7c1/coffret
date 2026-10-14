---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1014-0341-fill-a-folder-whose-first-page-is-present-and-read-fronts-ahead-of-the-first-page
created_at: 2026-10-13T18:41:00Z
updated_at: 2026-10-14T05:55:32Z
---

# perf(fetch): fill a folder whose first page is already present, read Container fronts when a folder is listed, and close two small windows in what a served fetch remembers

## Overview

Four findings from measuring parcel-based reading (PRs #271–#273). Two are
about when the server starts reading ahead; two are about what it remembers
and logs.

### 1. Opening a present page still starts the folder's fill

A fill is armed only by a `GET /api/file` that had to fetch
(`coffret-server/src/routes/file.rs:103-139`: `EntryFetch::Placed` arms,
`AlreadyPresent` does not, and `:61-63` answers a present file before any
fetch). When two books share a Pack, the second book's first page is often
placed alongside the first book's parcel; opening that book then fetches
nothing, nothing arms, and the reader waits about three seconds at the first
page that is not present. The doc in `fill/mod.rs:12-16` and `file.rs:104-109`
calls the necessity of a fetch "the whole of the signal".

Make the signal "a page was opened in a folder that is not wholly present":
on the present path, if the folder holds any mapped Entry that is not present
(a cheap catalog query beside `state_of`; add one to the device/usecase
Library if none answers it), arm the same fill `Placed` arms, with no
degraded or unheld findings to carry. A folder that is wholly present arms
nothing, so the common case costs one query. Update the two doc comments.
Add to `tests/routes/fill.rs` the case where the first page is placed by an
earlier fetch, the folder is then opened at that page, and the rest comes
over.

### 2. The fronts of a listed folder's Containers are read before a page is opened

The first page of a book not yet read costs three round trips: the header,
the meta section, and the parcel (`parcel_read/front.rs:31-44` reads the
front in two ranged reads; `outline()` at `:52-74` serves it from
`FetchMemory::front` when remembered). A listing (`GET /api/list`,
`routes/list.rs`) reads the catalog only and touches no Container. When a
folder is listed, read the fronts of the Containers its Entries live in, in
the background, bounded to a handful of distinct Containers per listing (the
first eight in listing order; a book is one Pack, so one or two is typical),
skipping those `FetchMemory` already holds, and remember them with
`remember_front`. The reads go through the same store and the same
`FetchMemory` the Entry fetch uses, so a page opened afterwards costs the
parcel alone. The front names no Entry (PK-16, PK-20), so what the provider
observes of a listing is that the folder's Containers were opened, which a
fill makes it observe anyway. Hang it where `OpenLibrary::list` is answered
(a device or usecase method the route calls after answering, in a spawned
task that never delays the listing), and add a route test in
`tests/routes/remembered.rs` style: after a listing, the first Entry of a
Container in that folder reads its parcel and no front.

### 3. The stale-catalog refusal names its variant in the log

`refresh/when_stale.rs:47-51` logs "the Container the catalog names is not
on Storage" for both refusals `may_mean_catalog_stale` matches
(`fetch_error.rs:419-426`): `NotFound` (Storage says it holds no such
object) and `ContainerUnreachable` (no handle reaches it at all). Give the
event a structured field naming which (`refusal = "not_found" |
"unreachable"`) and a sentence that fits both, and assert the field in the
`catch_ups_for_a_fetch` helper's tests (`tests/routes/remembered.rs:230-275`).

### 4. A key table read before a repair cannot be remembered after it

`remember_key_table` (`fetch_memory/mod.rs:170-181`) overwrites what it
holds; the write runs call `forget_key_table` after committing or repairing
(`run_sync.rs:63`, `run_freeze.rs:119`, `run_delete.rs:60`). A served fetch
that read the replica set before the repair (`entry_run.rs:282-300`) and
reaches `remember_key_table` after the forget puts the pre-repair finding
back, and the set is said to be degraded until the next catch-up. Give
`FetchMemory` a generation that every forget advances; `key_table()` takes
the generation before reading the replicas and passes it to
`remember_key_table`, which ignores a table read under an older generation.
A unit test beside `a_container_no_longer_current_is_forgotten`
(`fetch_memory/mod.rs:220`) covers the ignored remember, and a route test
beside `a_sync_that_repaired_the_set_stops_the_degraded_finding`
(`tests/routes/remembered.rs:356`) covers the overlap.

### 5. The work answer fixture carries `kept_parcels`

`frontend/packages/apps/web/src/useWork.test.tsx` builds a work answer whose
`catalog: { state: 'caught_up', stopped: null },` is not followed by the
`kept_parcels` field the answer now carries. Add `kept_parcels: null,` after it
so the fixture matches the answer's shape.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `tests/routes/fill.rs`: opening a present first page of a folder with
      absent Entries arms a fill that brings the rest over; opening a page of
      a wholly present folder arms nothing
- [x] `tests/routes/remembered.rs`: after `GET /api/list` of a folder, the
      first Entry fetched from a Container of that folder reads its parcel
      only (no header, no meta); a listing of a folder whose fronts are
      remembered reads nothing from Storage; the listing itself answers
      without waiting for the front reads
- [x] The stale-catalog catch-up log event carries `refusal = "not_found"` or
      `"unreachable"` according to the refusal, asserted in the existing
      catch-up tests
- [x] `fetch_memory` unit test: a table remembered under a generation older
      than the last forget is ignored; a route test shows a fetch overlapping
      a repairing sync does not bring the degraded finding back
- [x] `useWork.test.tsx`'s work answer fixture carries `kept_parcels: null`
- [x] `make check` passes

### Before merge (verified outside the check command)

- [ ] On the agent's Library, second device (`.tmp/agent/up.sh 2`, headless
      Chromium): with `lib/book-000` not yet read, list the folder, wait two
      seconds, open page one, and the server log shows one parcel read and
      no front read for that open; open page one of a folder whose first
      page is present but the rest is not, and a fill starts without a
      second page being opened (the log and `GET /api/work` show it)
