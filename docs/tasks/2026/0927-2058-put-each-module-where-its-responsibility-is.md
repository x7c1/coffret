---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && B=backend/crates && S=$B/apps/coffret-server && ! test -e $S/tests/routes.rs && test -f $S/tests/routes/main.rs && ! test -e $S/src/routes/work.rs && test -f $S/src/routes/work/mod.rs && [ "$(wc -l < $S/src/api_error/mod.rs)" -le 300 ] && ! grep -rq "ListingLimitReached" $B/ && ! grep -q "DRIVE_FILE_SCOPE" $B/gateway/google-drive-store/src/oauth/token_endpoint.rs && ! test -e $B/gateway/google-drive-store/src/app_folder.rs && ! grep -q "fn to_grouped_string" $B/domain/coffret-format/src/recovery_code/mod.rs && ! grep -q "fn now" $B/apps/coffret-device/src/batch_id.rs && [ "$(grep -rl "fn stub_endpoint" $B/ | wc -l)" -eq 1 ] && ! grep -rq "fetchedElsewhere" frontend/packages/apps/e2e/ && bash -n scripts/e2e-it.sh'
assignee: null
branch: task/0927-2058-put-each-module-where-its-responsibility-is
created_at: 2026-09-27T20:58:00Z
updated_at: 2026-09-27T21:32:21Z
---

# refactor: put each module, test and check where its responsibility is

## Overview

Several modules hold more than one responsibility, one responsibility is spread over more than one place, and a few leftovers of earlier changes no longer serve anything. This change moves each to where its responsibility is. There are no behaviour changes, no wire changes and no changes to what the tests assert, except where a check is removed as a duplicate or with the dead code it covered.

Paths below are relative to `backend/crates/` unless they start with `frontend/`, `scripts/` or `docs/`.

1. **Split the coffret-server route tests by flow.** `apps/coffret-server/tests/routes.rs` is one 3,800-line target of about 100 cases, in implicit sections. Keep one test binary, so there is one link and full parallelism, and make it a directory target:
   - `tests/routes/main.rs` holds the module doc and one `mod` per flow: listing, file, mapping, work, fill, upload (with sync), refresh, freeze, fences, lock, redaction, contract.
   - Move `tests/support/` under it, with the JSON readers the flows share (`files`, `states`, `listing_of`, `folders*`, `fill`, `sync`, `freeze`, `declined`, `written`, `rows_of`) in one support module.
   - The answers.json contract case keeps its helpers in `contract.rs`.
   - Update the doc reference in `apps/coffret-server/src/authorize/tests.rs` that names `tests/routes.rs`.
   - Move the cases unchanged: the same names, the same assertions, the same count.
2. **Split `routes/work.rs` one DTO per file.** It holds 11 DTO structs and their `impl`s (about 580 lines). Follow `routes/upload/`, which keeps one DTO per file:
   - `routes/work/mod.rs` keeps the handler, the `mod` declarations and the shared helpers (`STOPPED`, `named_folders`).
   - Each DTO gets its own file.
   - `RefusalDto` is shared with upload, so it moves up to `routes/refusal_dto.rs`.
   - Fields and `of` functions become `pub(super)` where a sibling reads them.
3. **Split `api_error/mod.rs` by responsibility.** It is about 670 lines, with one `impl ApiError` holding about 20 constructors. `api_error/refused_root.rs` is already an `impl ApiError` in its own file, and is the precedent.
   - `mod.rs` keeps the struct, the kind constants, the accessors and the private builders every constructor uses.
   - Group the constructors into sibling files by what they answer: admission (unauthorized, locked, no such route or method), paths (bad path, no such entry), declines, placements, the drop's budget, server. Make helpers `pub(super)` where siblings call them.
   - The goal is a `mod.rs` of at most about 300 lines.
4. **Remove the listing-cap variants nothing raises.** Since uploads are verified from each write's own answer, nothing constructs `SyncError::ListingLimitReached` or `FreezeError::ListingLimitReached`. The variants' own docs say so.
   - Remove both variants, with their `Display`, `source` and `redacted` arms.
   - Remove the `from_sync` / `from_freeze` arms in `apps/coffret-server/src/api_error/from_error.rs`, and the doc prose there that speaks of the flows' own caps.
   - In `api_error/tests.rs`, remove the case `a_listing_past_its_cap_says_so_from_both_flows_that_list`. Rewrite the `flows_own` baseline in `a_listing_the_storage_port_says_ran_past_its_cap_is_answered_the_same_way` to compare against the sentence directly.
   - Keep `listing_ran_past_its_cap`: `StorageError::ListingPastCap` still maps to it.
5. **Module split candidates in the device, CLI and Drive gateway.**
   - **The clock.** `now()` lives in `apps/coffret-device/src/batch_id.rs` beside `next_batch_id`. Move it to its own module (e.g. `device_time.rs`) and follow its four importers.
   - **`stub_endpoint`.** It is written twice: `apps/coffret-device/src/testing/mod.rs`, which is `cfg(test)`, and `apps/coffret-cli/tests/support/stub_bucket.rs`. Keep one copy, reachable from both. A dev-only feature on coffret-device that exposes the std-only stub, enabled from coffret-cli's `[dev-dependencies]`, is the cheaper option. Under resolver 2 a dev-dependency feature does not reach normal builds. The AWS signing credentials the two copies set differently stay with each caller.
   - **Drive constants.** `apps/coffret-device/src/testing/drive_stub.rs` redefines the token endpoint and the Drive scope that `google-drive-store` exports. Use the exported ones.
   - **`app_folder.rs`.** `gateway/google-drive-store/src/app_folder.rs` (476 lines) holds two public functions, `create_app_folder` and `read_app_folder_name`, and their tests. Split it into one file each, and update the `mod` / `pub use` pairs in `lib.rs`.
6. **`DRIVE_FILE_SCOPE` lives where it is used.**
   - It is defined in `gateway/google-drive-store/src/oauth/token_endpoint.rs`, which never uses it. Move it, with its SA-3 / SA-4 / SA-7 doc, to `oauth/granted_scopes.rs`.
   - Keep the public paths through `oauth/mod.rs` and `lib.rs` unchanged.
   - Point the two internal imports at `crate::oauth::DRIVE_FILE_SCOPE`, and drop the test-only duplicates `DRIVE_SCOPE` in `authorization/tests.rs` and `granted_scopes.rs`.
7. **One operation per file in `recovery_code/`.** `to_grouped_string` is the only operation left in `domain/coffret-format/src/recovery_code/mod.rs`; its siblings `encode.rs` and `parse.rs` each hold one. Give it its own file.
8. **Build the S3 client once for a join.**
   - `apps/coffret-device/src/s3.rs` `check_bucket` and `check_library_object` each build an SDK client. `join_library/run.rs` calls both, so the config and credential resolution run twice.
   - Let the checks take a `&Client` that the caller builds once. `create_library` calls only `check_bucket`, and builds its own.
   - Keep the module doc ("Three callers, one client") true.
9. **Check the freeze round trip once.**
   - `scripts/e2e-it.sh` stage 1 and journey `frontend/packages/apps/e2e/journeys/07-freeze.spec.ts` both check the cross-device round trip of a frozen book: fewer Containers than pages. The CLI's `fetched N, containers M` line is parsed twice as well, by sed in the script and by `fetchedElsewhere` in `journeys/uploader.ts`.
   - Stage 1 already owns the Storage-side facts, as its own comment says. So keep the round trip there. Remove the journey's copy, `fetchedElsewhere` and the environment fields only it used, and rewrite journey 07's header to say what the journey checks.
   - `make check` typechecks and lints the e2e package but does not run it. CI's `e2e` job does.
10. **Small consistency fixes in files this change does not otherwise reach.**
    - In `domain/coffret-usecase/src/commit_conformance/mod.rs`, `mod rival_index;` sits before `mod refusals;` / `mod repair;` — the only inversion in that list.
    - `apps/coffret-server/src/sync/progress.rs` groups its imports super-then-crate with no blank line, unlike its `sync/` and `fill/` / `freeze/` siblings.
    - `apps/coffret-device/src/browse/folders.rs`'s `debug!` lacks the `operation` / `library` fields `browse/list.rs` carries. `browse/container_of.rs` and `list.rs:194` carry `operation` but no `library`, so give them the same fields.

Guard: no behaviour, wire or format changes. Test cases move without changing their names or assertions. The only exceptions are the case removed in item 4 and the duplicated journey check removed in item 9. Public item paths (`coffret_server::…`, `google_drive_store::…`, `coffret_format::…`, `coffret_device::…`) stay as they are.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `tests/routes.rs` is replaced by the `tests/routes/` directory target, and `routes/work.rs` by `routes/work/mod.rs` (file gates)
- [x] `api_error/mod.rs` is at most 300 lines (line-count gate)
- [x] No `ListingLimitReached` remains under `backend/crates/` (grep gate)
- [x] `DRIVE_FILE_SCOPE` no longer lives in `token_endpoint.rs`, `app_folder.rs` is split, `to_grouped_string` left `recovery_code/mod.rs`, `now` left `batch_id.rs`, and `stub_endpoint` is defined once (file and grep gates)
- [x] The e2e journeys no longer parse the CLI's fetch line (`fetchedElsewhere` is gone; grep gate), and `scripts/e2e-it.sh` still parses (`bash -n`)
- [x] `make check` passes, including the moved route tests
