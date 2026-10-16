---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, error-type-design, rust-module-structure]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1017-0248-let-a-page-fetch-wait-for-a-front-already-being-read-ahead
created_at: 2026-10-16T17:48:44Z
updated_at: 2026-10-16T18:34:08Z
---

# perf(fetch): let a page fetch wait for the listing, key table and Container front a folder's read-ahead is already reading

## Overview

Listing a folder starts a read-ahead of its Containers' fronts (`read_fronts_ahead` in `backend/crates/apps/coffret-server/src/fronts_ahead.rs` → usecase `read_fronts` in `domain/coffret-usecase/src/fetch/fronts_ahead.rs`): it reads the Storage listing, the Keyring's key table, and each Container's header and meta section, and keeps them in `FetchMemory`. A page opened while that read-ahead is still running misses the cache and reads the same three things again (`fetch_entry` in `fetch/entry_run.rs`: `listed`, `key_table`, then `read_entry` → `front::outline` in `fetch/parcel_read/front.rs`). Measured on Drive, a book opened right after its folder took 7 round trips and 5.4 s instead of 1 round trip and ~1.5 s.

Coalesce the reads:

- `FetchMemory` keeps, beside what it has read, what is being read: one in-flight entry for the listing (keyed by head generation), one for the key table (keyed by its commitment), and one per Container front (keyed by `ContainerId`). A reader that finds an in-flight entry awaits it instead of reading; the first reader reads and publishes the result (or its failure) to every waiter, then the entry is removed.
- Both the read-ahead and the page fetch go through `listed`, `key_table` and `front::outline`, so the coalescing lives there and covers both directions (a read-ahead that finds a fetch already reading a front waits for it too).
- A failure is delivered to the waiters as the same typed error the reader got; a waiter does not retry on its own behalf beyond what the reader's error already says. A cancelled reader (its task dropped) must not leave waiters hanging: the in-flight entry is cleared and a waiter falls back to reading itself.
- Add debug-level counters or log fields for listing, key-table and front reads (today only parcel reads are counted, as `read_from_storage`), so the round trips of one page open can be read off the log.

Out of scope: changing what the read-ahead reads or how many fronts it reads.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A usecase test with a store that counts reads and blocks on demand: a page fetch started while the read-ahead is reading the same Container's front performs no listing, key-table, header or meta read of its own and returns the page.
- [x] The converse: a read-ahead that meets a fetch already reading a front does not read it again (test).
- [x] A failing in-flight read hands its error to its waiters, and a dropped reader does not leave a waiter hanging — the waiter reads for itself (tests).
- [x] The debug log of a page open names how many listing, key-table, front and parcel reads it made (test on the logged fields or a counter).

### Before merge (verified outside the check command)

- [ ] On the agent's two-device MinIO Library with the server at debug level: opening a not-yet-fetched book immediately after listing its folder logs one parcel read and no second front, listing or key-table read.
