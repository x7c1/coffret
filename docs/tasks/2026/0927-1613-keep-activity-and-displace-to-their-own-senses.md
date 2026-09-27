---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqF "account of its activity" docs/concepts/ && ! grep -rqiE "\bdisplac(e|es|ing)\b" backend/crates/domain/ && ! grep -rqiE "(rather than|instead of) displacing" --exclude-dir=target --exclude-dir=node_modules --exclude-dir=dist backend/crates/ frontend/packages/'
assignee: null
branch: task/0927-1613-keep-activity-and-displace-to-their-own-senses
created_at: 2026-09-27T16:13:00Z
updated_at: 2026-09-27T17:05:00Z
---

# docs: keep activity and displace to their own senses

## Overview

Two words are used in prose in a sense the spec or the concept documents give to another word. A reader who trusts that the concept documents, the spec and the code name the same thing with the same word draws the wrong conclusion at each of them. Every fix here is prose — one concept-document sentence and doc / test comments — and none changes behaviour or any identifier.

1. **`activity` for the answer to "what is this device doing".** DK-4 in `docs/spec/device-key-custody/README.md` defines activity as "the span of a keyed operation and not the moment a request arrived", and says "a request that needs no key is not activity, since an open window asking what a device is doing is not a person at the keyboard". The Master Key concept (`docs/concepts/master-key/README.md`, around line 39) says an explorer "learns of the lock from the device's own account of its activity" — the one keyless request DK-4 names as not activity. LA-12 in `docs/spec/loopback-access/README.md` calls what that request answers with "what a server holds about the work it runs". Say *the device's own account of its work* (or an equivalent in LA-12's words), so the concept no longer calls the asking activity.
   - Out of scope here: the route `/api/activity`, the types `ActivityDto` / `Activity` / `SyncActivity` / `FreezeActivity`, and the doc comments that describe those types by their current names. They are renamed together in a separate change that also moves the wire; this change touches no identifier.
2. **`displace` for one thing taking another's place.** *Displaced* is kept for one sense: a run that had already stopped and whose place on record a later run took (`displaced` in `backend/crates/apps/coffret-server/src/latest.rs`, the `displace` functions in `fill/progress.rs` and `freeze/progress.rs`, `DISPLACED_KEPT`). The prose also uses the verb for two things the concept documents register under *supersede*:
   - **A run taking a running run's place.** The Library concept registers "supersede (a fill in progress, by a fetch in another folder that puts the fill there instead) — one run taking another's place" (`docs/concepts/library/README.md`, Collocations). The comments that say a folder asked for by name "waits rather than displacing" the run in progress are about that: `coffret-server/src/routes/activity.rs` (around lines 142 and 226), `routes/fill.rs:27`, `fill/fill_folder.rs:25`, `routes/upload/mod.rs:179`, `freeze/freeze_status.rs:11`, `fill/progress.rs` (around lines 64, 119, 314), and on the frontend `apps/web/src/fill.ts` (around lines 160, 473), `apps/web/src/FileList.tsx` (around lines 43, 181), `apps/web/src/fill.test.ts:689`, `gateway/api/src/activity.ts` (around lines 115, 430). Say *superseding* (or say what happens — it waits its turn — without the verb) wherever the sense is a running run's place.
   - **One Container taking another's place.** The Container concept's *superseded* is one Container taking another's (`docs/concepts/container/README.md`, "trash (a superseded Container)"; the Library concept's supersede entry names the pairing). The domain crates say a Container "displaces" the ones it replaces: `coffret-usecase/src/spooled_container.rs` (around lines 21, 60–65), `coffret-usecase/src/freeze/selected.rs:6`, `coffret-usecase/src/freeze_conformance/absorption.rs:30`, `coffret-usecase/src/commit/journal.rs:72`, `coffret-model/src/journal_record/new.rs:65`, `coffret-model/src/journal_record/tests.rs:95`. Say *supersedes*.
   - Read each remaining use of the verb (for example the comment near `coffret-server/tests/routes.rs:1229`) and decide by its sense: the stopped-run record keeps *displace*; a running run's or a Container's place takes *supersede*.

Guard: no identifier changes — `displaced`, `displace`, `DISPLACED_KEPT`, `replaces`, and every `Activity` type and route keep their names. The spec text of DK-4 and LA-12 does not change; the concept aligns to it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `docs/concepts/` no longer contains "account of its activity" (grep gate)
- [x] No `displace` / `displaces` / `displacing` remains under `backend/crates/domain/` (grep gate): the domain crates hold no stopped-run record, so every use there was a Container's supersession
- [x] No "rather than displacing" / "instead of displacing" remains under `backend/crates/` or `frontend/packages/` (grep gate)
- [x] `make check` passes
