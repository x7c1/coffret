---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rq "DEVICE_SCHEMA_VERSION: i64 = 7" backend/crates/gateway/coffret-sqlite-index/tests/ && grep -q "head-0" docs/concepts/library/README.md && ! grep -q "let go" backend/crates/domain/coffret-usecase/src/fetch/unheld_parcel.rs && grep -q "trash_folder" docs/spec/entry-path/README.md && grep -q "reconnect" docs/spec/storage-authorization/README.md && grep -q "5 seconds each" docs/spec/loopback-access/README.md'
assignee: null
branch: task/1023-1840-bring-seven-documents-and-wordings-back-in-line-with-the-code
created_at: 2026-10-23T09:40:00Z
updated_at: 2026-10-23T14:40:00Z
---

# docs: bring seven documents and wordings back in line with the code

## Overview

Seven places where a document, a doc comment or an output line fell behind the
code, collected from review. Each is one decision already taken; apply them as
written. Paths are relative to the repository root. Where a spec rule's text
changes, it is to describe what the code does today, not to change behaviour.

### 1. SQLite Index: test constants and the `prepare` doc

`backend/crates/gateway/coffret-sqlite-index/src/schema/mod.rs:34` has
`SCHEMA_VERSION = 19` and `:78` `DEVICE_SCHEMA_VERSION = 18`, but
`tests/schema.rs:59` and `tests/refused_index.rs:26` still define
`DEVICE_SCHEMA_VERSION: i64 = 7`. Set the test constants to the current value,
re-read each case that uses them (`refused_index.rs:71,96,105,115,134`,
`schema.rs:608`) and make sure each still exercises what its name says — a case
that only passed because the constant was stale is fixed, not deleted. Re-read
the `prepare` doc (`schema/mod.rs:325-352`) and `within` (`:362-366`) against
the current match arms (17 → reservations, 16 → entry identities, 7..=15 →
provenance) and correct any sentence that no longer holds.

### 2. Library concept: "create" in Collocations

`docs/concepts/library/README.md` Collocations (from line 127) has no entry
for creating a Library, although the Journal concept
(`docs/concepts/journal/README.md:43-45`) and CP-2 treat creation as writing
the first head, `head-0`, to Storage. Add the entry in the section's own style.

### 3. LA-19 says "each"

`docs/spec/loopback-access/README.md:292-297` (LA-19) says reading a request
to `/auth` stops 5 seconds after it began, while the host gives the head and
the body 5 seconds each (`coffret-explorer-host/src/lib.rs:92-94`
`READ_DEADLINE`, used at `serve.rs:55` for the head and `auth/launch.rs:45`
for the body). Rewrite LA-19 to say the head and the body are given 5 seconds each
(use the exact phrase "5 seconds each"; a check gate greps for it), and align
the `READ_DEADLINE` doc comment. Tests citing LA-19
(`tests/auth.rs`, `tests/serve.rs`, `tests/admission.rs`) keep their citations;
`make spec-rule-ids` must still pass.

### 4. Library concept: browser token expiry matches LA-18

`docs/concepts/library/README.md:415-416` says a browser token lasts "until
the token expires, is revoked, or the host restarts"; LA-18
(`loopback-access/README.md:283-285`) also discards it when the server
restarts and when the Library served changes. Add the two conditions.

### 5. EP-16, the Library concept and the storage-authorization spec

- The Library concept has no rule that the desktop's trash folder is excluded
  from the Library; add it where the concept describes what a mapping brings
  in, citing EP-16.
- EP-16 (`docs/spec/entry-path/README.md:654-`) covers scan and fetch but not
  that a drop into a trash folder is declined as `trash_folder`
  (`coffret-server/src/api_error/from_error.rs:506-517`,
  `tests/routes/upload.rs:571`). Add that sentence.
- `docs/spec/storage-authorization/README.md` (SA-1..SA-7) does not describe
  reconnect. Add the promise the code keeps
  (`coffret-server/src/routes/work/reconnect_dto.rs`): the work answer for a
  reconnect carries `flow`, a digest of the consent page's URL, only while the
  reconnect is waiting, and never the page or its URL. Give it a rule id in
  the file's sequence and cite it from `reconnect_dto.rs`.

### 6. `unheld_parcel.rs` does not say "let go"

`backend/crates/domain/coffret-usecase/src/fetch/unheld_parcel.rs:9` says an
unheld parcel "is let go and asked of Storage again". The Index concept
(`docs/concepts/index/README.md:39-42`, Collocations `:171-172`) reserves "let
go" for a held parcel. Say it differently (dropped from memory, read again).

### 7. The CLI's text output tells `trashed` from `never_uploaded`

`backend/crates/apps/coffret-device/src/finding.rs:463-470` prints one sentence
for `Disposal::NeverUploaded | Disposal::Trashed`. Split the arm: one says the
Container never reached Storage, the other that what it left in Storage was
moved to the trash. JSON (`coffret-cli/src/answer/found.rs:150-158`) already
tells them apart; the text tests at `report.rs:303-310,336-339` are updated.

### 8. Library concept: a folder standing on disk under a mapped folder

The explorer's drop confirmation now tells a folder that stands on this
device's disk under a mapped folder but that the Library does not hold (the
listing's `folders_on_disk`) from a folder that is new, since the run that
carries the drop in also carries in what already stands there. The Library
concept's Domain Rules (around "A folder standing in a mapped folder with no
Entry under it is not a folder of the Library…") describe such a folder but
give it no name and do not say what a drop onto it does. Add one sentence
there: such a folder is not a prospective folder — a drop let go over it does
not make it, and the run that carries the drop in carries in what already
stands in it.

### Out of scope

- Any behaviour change behind these texts.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] No test under `coffret-sqlite-index/tests/` defines `DEVICE_SCHEMA_VERSION` as 7 (gate appended to `check_command`), and the suite passes with the current value.
- [x] `docs/concepts/library/README.md` Collocations has a create entry naming `head-0` (gate appended).
- [x] LA-19 says the head and the body are given "5 seconds each" (`grep -q "5 seconds each"` on the file is appended; `make spec-rule-ids` and `make spec-citations` pass).
- [x] The Library concept's browser-token sentence lists server restart and Library change.
- [x] EP-16 mentions the `trash_folder` decline (gate appended), the Library concept excludes the trash folder, and the storage-authorization spec has a rule for the reconnect work answer's `flow` (`grep -q "reconnect"` on that spec is appended) cited from `reconnect_dto.rs`.
- [x] `unheld_parcel.rs` no longer says "let go" (gate appended).
- [x] `coffret-cli` report tests: a `trashed` settlement and a `never_uploaded` one print different sentences.
