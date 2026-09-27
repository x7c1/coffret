---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && X="--exclude-dir=target --exclude-dir=node_modules --exclude-dir=dist --exclude-dir=dist-types" && ! grep -rqF $X "/api/activity" backend/crates/ frontend/packages/ scripts/ && ! grep -rqwE $X "ActivityDto|SyncActivity|FreezeActivity|useActivity|getActivity" backend/crates/ frontend/packages/ && grep -rqF $X "\"/api/work\"" backend/crates/apps/coffret-server/src/ && grep -rqwE $X "FillRun" backend/crates/apps/coffret-server/src/ && grep -rqwE $X "SyncRun" backend/crates/apps/coffret-server/src/ && grep -rqwE $X "FreezeRun" backend/crates/apps/coffret-server/src/ && grep -rqF $X "refused_placement" backend/crates/apps/coffret-server/src/api_error/ && grep -rqF "refused_placement" frontend/packages/gateway/api/src/contract/refusals.json && ! grep -rqF "\"dropped\":" frontend/packages/gateway/api/src/contract/ && ! grep -rqF "\"trouble\":" frontend/packages/gateway/api/src/contract/ && grep -qE "^- displace " docs/concepts/library/README.md'
assignee: null
branch: task/0927-1641-name-the-work-answer-and-its-refusals-by-the-registered-words
created_at: 2026-09-27T16:41:00Z
updated_at: 2026-09-27T17:24:17Z
---

# refactor: name the work answer and its refusals by the registered words

## Overview

The answer a browser polls, the runs it reports, and some of the refusals it carries use words that the spec and the concept documents give to other things. A reader who trusts that the concept documents, the spec and the code name the same thing with the same word draws the wrong conclusion at each of them. This change renames the identifiers and the wire values together, since the server and the explorer ship from this one repository and nothing outside it reads the wire. It also registers two words in the Library concept that the code already relies on.

1. **The poll is not activity.** DK-4 (`docs/spec/device-key-custody/README.md`) defines activity as "the span of a keyed operation", and says "a request that needs no key is not activity, since an open window asking what a device is doing is not a person at the keyboard". Yet that keyless request is `GET /api/activity`, answered by `ActivityDto` (`backend/crates/apps/coffret-server/src/routes/activity.rs`) and read as `Activity` (`frontend/packages/gateway/api/src/activity.ts`). The word also names two different things: Rust `fill::Activity` is one fill run, and TS `Activity` is the whole answer. LA-12 (`docs/spec/loopback-access/README.md`) calls the answer "what a server holds about the work it runs", and a run of each kind "the last run of each kind". Rename the pieces as follows:
   - **The answer.**
     - Route `/api/activity` → `/api/work` (`src/router.rs`).
     - `ActivityDto` → `WorkDto`.
     - `routes/activity.rs` and `routes/activity/contract.rs` → `routes/work.rs` and `routes/work/contract.rs`.
     - The contract fixture `gateway/api/src/contract/activity.json` → `work.json`.
     - On the frontend: `activity.ts` → `work.ts` (and its test); `Activity` → `Work`; `getActivity` → `getWork`; `useActivity` (`apps/web/src/useActivity.ts`, and its test) → `useWork`.
     - Every caller, including the e2e journeys and `scripts/` wherever they name the route.
   - **The runs.**
     - `fill::Activity` (`src/fill/activity.rs`) → `FillRun` (`fill/fill_run.rs`).
     - `SyncActivity` (`src/sync/sync_activity.rs`) → `SyncRun` (`sync/sync_run.rs`).
     - `FreezeActivity` (`src/freeze/freeze_activity.rs`) → `FreezeRun` (`freeze/freeze_run.rs`).
     - Local variables, fields and doc comments that call the answer or a run "activity" follow. This includes the doc comments in `coffret-device/src/run_sync.rs:25`, `coffret-device/src/run_freeze.rs:60` and `coffret-usecase/src/progress.rs:114`, which speak of "the activity a browser polls".
   - Guard: `coffret-server/src/lock/mod.rs` uses *activity* in DK-4's own sense (the poll "takes no key and so is not activity"), and so does any other place meaning a keyed operation's span. Those keep the word.
2. **`dropped` is not the concept's drop.** The Library concept registers *drop* as "files a browser drops into a mapped folder" (`docs/concepts/library/README.md`, Collocations), and `apps/web/src/dropped.ts` uses it that way. The wire field `dropped` means something else: the folders a worker that ended without an answer threw away from its queue (`src/latest.rs`, `Latest::dropped`; `FillDto` / `FreezeDto` in `routes/activity.rs`; `Fill.dropped` / `Freeze.dropped` in `gateway/api/src/activity.ts`). Rename that field `discarded`, on both sides of the wire and in every reader (`StatusBar.tsx`, `retry.ts`, `dismissed.ts` and their tests). The browser's drop — `dropped.ts`, `DroppedFile` and the rest — keeps its words.
   - *abandoned* was not chosen: OC-3 and the Journal concept use it for a batch given up before commit, and a freeze has batches.
   - *lost* was not chosen: the Keyring concept uses *key-lost*.
3. **Kind `declined` carries more than a decline.** The Entry Path concept (`docs/concepts/entry-path/README.md`, Collocations) registers two words. *decline* means "to place an Entry, reporting the reason" — a fetch's per-Entry verdict. *refuse* is the wider verdict, which covers "a placement the device will not make, whether of the one file at that path or of every file under a mapped root". Its Domain Rules add that a writer "declines each placement refused for the first reason … and fails the whole request on the second". The server sends three refusals of the wider kind as kind `declined`:
   - `ApiError::refused_root` (`api_error/refused_root.rs`), a mapping's root, which stops a whole run;
   - `ApiError::no_folder_here` (`api_error/mod.rs`), a drop or a freeze under a folder this device has no folder for;
   - `ApiError::pack_resident` (`api_error/mod.rs`), a drop that would replace an Entry inside a Pack.

   Give these three a new kind, `refused_placement` (still 409), with their reasons unchanged. A fetch's per-Entry verdicts stay `declined`: the reasons `unmapped`, `unmaterializable`, `reserved`, `surfaced` and `locked` (`declined_as`, and the `surfaced` / `reserved` constructors). Then update what depends on the kind:
   - the kind list in `ApiError`'s doc;
   - `RefusalKind` and the reason type in `gateway/api/src/refusal.ts` / `activity.ts`. If one reason type now spans both kinds, name it for what it carries, and fix "Present exactly where the kind is `declined`";
   - `retry.ts`'s branch on `reason === 'refused_root'`;
   - the golden fixture `gateway/api/src/contract/refusals.json`, regenerated with `COFFRET_WRITE_CONTRACT=1`;
   - the route tests that assert `"declined"` for these three.

   *refused* alone was not chosen: EL-1 (`docs/spec/event-logging/README.md`) calls every person-facing answer that refuses an operation a refusal, so a kind named `refused` would distinguish nothing.
4. **The catalog's `trouble` is the runs' `stopped`.** The catalog side of the answer carries `trouble: Refused | null` for what stopped the last catch-up (`gateway/api/src/activity.ts`, the catalog DTO in `routes/activity.rs`; the server already holds it as `Standing::Behind(Reported)` in `src/refresh/standing.rs`). Each run carries the same refusal as `stopped`. Rename the catalog field `stopped` on the wire and in its readers (`apps/web/src/refresh.ts`). The web app's other `trouble` names (`retry.ts`'s `Trouble`, the `trouble` callbacks in `lock.ts` / `refresh.ts`) are a refusal the page met, not this field, and keep their names.
5. **Register what the code already relies on.** In `docs/concepts/library/README.md`, Collocations:
   - Add `displace (a run that had stopped, by a later run taking its place on record)`, placed beside *supersede* and set against it: supersede is a running run's place. The code keeps this sense in `Latest::displaced`, the `displace` functions in `fill/progress.rs` / `freeze/progress.rs`, and `DISPLACED_KEPT`.
   - After the *supersede* entry, add one clause saying what does not supersede: a freeze, or a folder asked for by name, waits its turn behind the run in progress. The code says so in those words (`routes/fill.rs`, `freeze/mod.rs`).

Guard: the spec text (DK-4, LA-12, EL-1, EP-11, EP-13) does not change; the code and the concept documents align to it. No behaviour changes beyond the renamed route, fields and kind — the same refusals are sent in the same cases with the same messages and reasons.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] No `/api/activity` remains under `backend/crates/`, `frontend/packages/` or `scripts/`, and the router serves `/api/work` (grep gates)
- [x] No `ActivityDto`, `SyncActivity`, `FreezeActivity`, `useActivity` or `getActivity` remains, and `FillRun`, `SyncRun`, `FreezeRun` exist in coffret-server (grep gates)
- [x] The kind `refused_placement` exists in `api_error/` and in the regenerated `refusals.json` fixture (grep gates); the contract tests on both sides pass against it (`make check`)
- [x] The contract fixtures under `gateway/api/src/contract/` carry neither a `dropped` nor a `trouble` field (grep gates)
- [x] The Library concept registers `displace` in its Collocations (grep gate)
- [x] `make check` passes
