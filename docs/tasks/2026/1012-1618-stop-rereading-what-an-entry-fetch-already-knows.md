---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1012-1618-stop-rereading-what-an-entry-fetch-already-knows
created_at: 2026-10-12T07:18:37Z
updated_at: 2026-10-12T08:20:06Z
---

# perf(fetch): stop asking Storage again for what a server's Entry fetches already know

## Overview

Every Entry fetch (`fetch_entry` in
`backend/crates/domain/coffret-usecase/src/fetch/entry_run.rs`) makes the same
four Storage round trips before it reads a single parcel:

1. a catch-up (`catch_up`), which lists the Library's folder on Drive;
2. the committed Keyring (`read_committed`), one replica read;
3. the Container's header (`parcel_read/front.rs`, 32 bytes);
4. the Container's meta section (`parcel_read/front.rs`).

A server repeats all four for every Entry it fetches, and the fill repeats them
for every stretch of parcels it moves on to inside the same Container. Measured
on a second device against Drive (2026-10-12), each round trip costs
0.4–1.4 s. The four of them take about 70% of the time to a book's first page
(3–9 s in total), and a reader who catches up with the fill waits about 3 s at
each parcel boundary, even though the parcel itself arrives in 0.3 s. None of
the four answers has changed between those fetches. This task removes the
repeated round trips. The spec does not require them: nothing in the spec says
an Entry fetch catches up first, and the catch-up rules (CK-9, RV-1) say how to
catch up, not when.

What a process may keep, and what makes each one stale:

- **The catalog's freshness** — a server already catches up at startup and
  when the explorer asks (`POST /api/refresh`; `coffret-server/src/refresh/`).
  An Entry fetch in the server answers from the catalog as it stands and does
  not catch up by itself. The one case it must handle is a catalog that is
  behind: Storage answers that the Container's object is not there because
  another device replaced or removed it. The fetch then catches up once and
  answers from the caught-up catalog: it places the Entry from its current
  Container, or says the Entry is no longer current. That fallback goes through
  the server's own catch-up path, so the catalog's `behind` / caught-up standing
  (LA, the work answer) stays truthful. The command line's `coffret fetch` runs
  once and keeps its one catch-up per run.
- **The listing and the key table** — kept from the last catch-up. They are
  replaced whenever a catch-up advances the head or the checkpoint's Keyring
  reference changes. The key table holds envelopes wrapped under the Library's
  wrap key. Even so, whatever holds it is dropped when the Library locks, the
  same as the keys (DK-2, DK-4), so a locked server holds nothing a reader could
  use.
- **A Container's front** (header and meta section) — a Container object never
  changes once committed (FM-2, CP-11). Its front is kept in memory per
  Container id, within a bound on how many fronts are kept, and dropped when the
  Container leaves the current set.

Where these live is a design choice for the work phase. The natural owner is
the per-process value the server already holds for Entry fetches
(`coffret-device/src/entry_fetches/`, which already gates fetches by Container),
handed into `FetchEntryRequest` so that the use case stays free of process
state. Nothing here changes the parcel rules (PK-16, PK-19 to PK-22). Every read
of chunks is still one whole parcel or the whole object. A front served from
memory shows Storage less, never more (PK-20). Update the module docs that
describe the old flow. `entry_fetches/mod.rs` says that both callers "catch the
catalog up, read the committed Keyring"; it must say what the process keeps
instead.

Out of scope: having a fill start for a folder whose first Entries were placed
alongside another Container's parcel, and
reading a Container's front ahead of the first request for it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A test over a counting Storage double: after one Entry fetch in a server process, a second Entry fetch from the same Container whose parcel is not held issues exactly one Storage read, its parcel. It issues no listing, no Keyring read, and no front read
- [x] A test: an Entry fetch from a second Container in the same process issues no listing and no Keyring read. It reads that Container's header, then its meta section, then the parcel
- [x] A test: once a catch-up advances the head (another device committed, changing the Keyring reference), the next Entry fetch reads the Keyring again, and it uses the newly committed key table
- [x] A test: when the catalog is behind and the Container's object is gone from Storage, the Entry fetch catches up once. It then either places the Entry from the Container now holding it or answers that the Entry is not current. It does not loop, and the catalog's standing reports the catch-up
- [x] A test: after the Library locks, the process holds no key table, and the first Entry fetch after unlock reads the Keyring again
- [x] A test: the fill moving on to the next stretch of parcels inside one Container issues only parcel reads
- [x] `coffret fetch` (CLI, one Entry) still catches up once per run (existing tests stay green)

### Before merge (verified outside the check command)

- [ ] On a second device (debug log), opening an unfetched 300-page book: the debug log shows no listing call and no Keyring read for the second and later fetches in the same server. The time to the first page and the wait at a parcel boundary are recorded in the PR. The boundary wait comes down from about 3 s to about one parcel round trip
