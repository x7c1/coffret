---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, spec-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it'
assignee: null
branch: task/1113-0310-open-the-delete-confirmation-without-storage
created_at: 2026-11-12T18:03:00Z
updated_at: 2026-11-12T20:44:22Z
---

# perf: count a deletion's preview from the catalog alone, without reaching Storage

## Overview

The explorer's "Delete…" asks `GET /api/delete` what a deletion would take
out and rebuild before it opens the confirmation dialog
(`frontend/packages/apps/web/src/DeleteConfirm.tsx`). On a Drive Library
that answer takes 1.2–1.5 s, all of it two Storage round trips:
`OpenLibrary::preview_delete`
(`backend/crates/apps/coffret-device/src/run_delete.rs`) first catches the
catalog up and then reads the committed Keyring (`committed_key_lost`) to
find the Packs whose key is lost, and only then runs the catalog-only
`coffret_usecase::preview_delete`.

Count the preview from the catalog as it stands and nothing else:

- `preview_delete` no longer catches up and no longer reads the Keyring or
  takes the keys. It runs the same plan the deletion runs, over the local
  catalog, with no Pack counted as key-lost — `deletes_anything` beside it
  already does exactly this.
- A Pack whose key is lost is therefore counted in the preview as one the
  deletion rebuilds. The deletion itself still catches up, reads the
  Keyring and refuses that Pack (spec: PK-10), and the refusal reaches the
  explorer's Activity as it does today; do not change the run.
- The preview's answer keeps its shape (fields and their meaning), so the
  explorer needs no change beyond what a now-always-empty or differently
  sourced field would need; if the refused list can no longer be filled
  from the catalog, keep it in the answer and say in the route's doc
  comment when it is non-empty.
- Remove "and the one before a deletion's preview" from the list of
  catch-ups in OC-8 (`docs/spec/orphan-cleanup/README.md`, the completion
  bullet), since there is no longer such a catch-up. Update every doc
  comment that says the preview catches up or reads the Keyring
  (`routes/delete.rs`'s `preview`, `run_delete.rs`'s `preview_delete`, and
  any other the change makes stale), and any spec text that says the
  preview names the Packs whose key is lost, so the rules say what the
  code now does. Do not change any other behaviour a rule describes.

Name the rules from the tests. Keep the HTTP status and refusal codes of
`GET` and `POST /api/delete` as they are. Do not change `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A test shows `GET /api/delete` answering from the catalog without
      reaching Storage: for example, against a store that fails or counts
      every call, the preview succeeds and makes no Storage call.
- [x] A test shows a Pack whose key is lost counted as rebuilt by the
      preview, while `POST /api/delete` of the same selection still refuses
      it (PK-10).
- [x] OC-8 no longer lists a catch-up before a deletion's preview, and no
      doc comment or rule says the preview catches up or reads the Keyring.
- [x] `make check && make e2e-it` passes.
