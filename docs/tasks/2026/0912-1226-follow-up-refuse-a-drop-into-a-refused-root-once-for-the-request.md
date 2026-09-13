---
status: completed
pipeline_phase: null
follow_up_of: docs/tasks/2026/0912-0455-refuse-a-drop-into-a-refused-root-once-for-the-request.md
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q ''is not the converse: a mapping this device holds may reach a folder'' frontend/packages/gateway/api/src/list.ts && grep -q ''An upload is the one flow that cannot promise the sentence arrives'' backend/crates/apps/coffret-server/src/api_error/mod.rs && grep -q ''they all go through this one root'' backend/crates/apps/coffret-device/src/error.rs && ! grep -qF "The last is an added file''s" frontend/packages/gateway/api/src/refusal.ts && ! grep -q ''upload that was handed one file'' backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs && ! grep -q ''This device is placing the one file it was'' backend/crates/apps/coffret-device/src/error.rs && grep -q ''A drop meets the first four'' backend/crates/apps/coffret-server/src/api_error/mod.rs'
assignee: null
branch: task/0912-1226-follow-up-refuse-a-drop-into-a-refused-root-once-for-the-request
created_at: 2026-09-12T12:27:04Z
updated_at: 2026-09-12T13:04:50Z
---
# docs: say that a drop meets these refusals too, not only a fetch

## Overview

Seven doc comments describe a refused root, or the set of reasons a placement
is declined, as though only a fetch or only a single handed file were involved.
Each is now false in a way a reader would act on: a drop meets four of those
reasons as well, and the upload route is handed many files rather than one.
Fix the text in place. **No behaviour changes** — every edit is a doc comment,
and no value the build or the runtime reads is touched.

1. `frontend/packages/gateway/api/src/list.ts`, lines 60-67 — extend
   `Listing`'s `mapped` doc with a paragraph saying `true` is not the converse:
   a mapping this device holds may reach a folder that is not the one it was
   recorded against, and a listing of it says `true` while every fetch and
   every drop into it is declined `refused_root`.

2. `backend/crates/apps/coffret-server/src/api_error/mod.rs`, line 239 — insert
   a paragraph into the `refused_root` doc saying an upload is the one flow
   that cannot promise the sentence arrives: it is answered while the browser
   is still sending, and a transfer that fails first leaves the browser saying
   the server did not answer instead. The log is what carries the case in that
   event.

3. `backend/crates/apps/coffret-device/src/error.rs`, lines 253-258 — rewrite
   the `RootRefused` variant doc so the reason the request fails as a whole is
   not "placing *one* file it was handed" — it names an upload, which hands
   several, as its instance — but that where several were handed they all go
   through this one root.

4. `frontend/packages/gateway/api/src/refusal.ts`, line 54 — replace the line
   so it reads that the first six are a fetch's and a drop meets `unmapped`,
   `unmaterializable`, `reserved` and `refused_root` as well, and the last is a
   drop's alone.

5. `backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs`, lines
   164-166 — replace "an upload that was handed one file" with "the upload
   route, however many files its drop was handed", and ground it on EP-13.

6. `backend/crates/apps/coffret-device/src/error.rs`, lines 1004-1006 —
   replace "This device is placing the one file it was handed" with "The
   request fails as a whole, however many files this caller was handed".

7. `backend/crates/apps/coffret-server/src/api_error/mod.rs`, lines 63-66, in
   the `reason` field doc anchored on "for a fetch (spec: EP-11)" — add the
   sentence "A drop meets the first four of those as well." immediately after
   "(spec: PK-10, PK-12).", so the paragraph no longer reads as though those
   six reasons are only a fetch's.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] `Listing`'s `mapped` doc says `true` is not the converse:
  `grep -q 'is not the converse: a mapping this device holds may reach a folder' frontend/packages/gateway/api/src/list.ts`
- [x] the `refused_root` doc names the flow that cannot promise the sentence
  arrives:
  `grep -q 'An upload is the one flow that cannot promise the sentence arrives' backend/crates/apps/coffret-server/src/api_error/mod.rs`
- [x] the `RootRefused` variant doc reasons from the shared root rather than
  from one handed file:
  `grep -q 'they all go through this one root' backend/crates/apps/coffret-device/src/error.rs`
- [x] the client's reason list no longer ends by calling the last one an added
  file's:
  `! grep -qF "The last is an added file's" frontend/packages/gateway/api/src/refusal.ts`
- [x] the fetch error doc no longer describes the upload route as handed one
  file:
  `! grep -q 'upload that was handed one file' backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs`
- [x] the device error doc no longer reasons from placing the one file it was
  handed:
  `! grep -q 'This device is placing the one file it was' backend/crates/apps/coffret-device/src/error.rs`
- [x] the `reason` field doc says a drop meets those reasons too:
  `grep -q 'A drop meets the first four' backend/crates/apps/coffret-server/src/api_error/mod.rs`
