---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && S=backend/crates/apps/coffret-server/src && W=frontend/packages/apps/web/src && G=frontend/packages/gateway/api/src && ! grep -qF "Storage did not answer" $W/fill.ts && ! grep -qF "the fill stopped before reaching this file" $W/fill.ts && ! grep -qF "the catch-up did not finish" $W/refresh.ts && ! grep -qE "askedForJson<(Work|Upload)>" $G/work.ts $G/upload.ts && ! grep -qF "no folder on this device holds" $W/unmapped.ts && ! grep -qE "\"(server|storage)\"" $S/reported.rs && ! grep -qF "\"unverified\"" $S/api_error/from_error.rs && [ "$(grep -rhoE "kind: \"(declined|refused_placement)\"" $S/api_error/ | wc -l)" -le 2 ] && [ "$(grep -c "\"unmapped\"" $G/contract/refusals.json)" -ge 2 ]'
assignee: null
branch: task/0927-1728-hold-the-work-answer-to-its-shape-by-type
created_at: 2026-09-27T17:28:00Z
updated_at: 2026-09-27T18:08:03Z
---

# refactor: hold the work answer and the refusals it carries to their shape by type

## Overview

The server and the explorer agree on the shape of the work answer (`GET /api/work`) and on the refusals it and the other routes carry. But several of those agreements hold only because each side happens to keep them. No type enforces them, so a page reads fields it cannot trust and fills the gaps with invented causes. One rule covers this change: a shape the two sides agree on is stated once, in a type or a single constructor, and each side reads it through that statement instead of assuming it. The wire shape itself does not change, except where noted in item 2.

Paths: `S` = `backend/crates/apps/coffret-server/src/`, `G` = `frontend/packages/gateway/api/src/`, `W` = `frontend/packages/apps/web/src/`.

1. **A stopped run always says what stopped it.** Every place that stops a run sets `status = Stopped` and `stopped = Some(..)` together, but no type ties the two:
   - `S/fill/run.rs` does it through `FillRun::stop`;
   - `S/sync/run.rs` and `S/freeze/run.rs` write the pair by hand;
   - each `abandon()` in `S/*/progress.rs` sets both with `Reported::unfinished()`.

   Make the stop carry its refusal. The status enums (`S/fill/fill_status.rs`, `S/sync/sync_status.rs`, `S/freeze/freeze_status.rs`) could get `Stopped(Reported)`, or each run type could hold the refusal inside a stopped state. The catalog's `Standing::Behind(Reported)` in `S/refresh/standing.rs` is the precedent. The DTO then derives `stopped` from the state, so the wire keeps `status` and `stopped` as two fields.
   - **What makes it harder.** The status enums derive `Copy, PartialEq, Eq`, and `Reported` is only `Clone`. The `finishes(progress, XStatus::Stopped)` test helpers build exactly the state this makes unrepresentable. The activity-contract builders in `S/routes/work/contract.rs` set `stopped: None` with struct-update overrides. Reshape all three.
   - **On the frontend,** make `Fill`, `Sync` and `Freeze` in `G/work.ts` discriminated unions, so that `status: 'stopped'` carries `stopped: Refused`. The catalog gets the same treatment: `stopped` is non-null exactly when the state is behind.
   - Delete the fallbacks that invent a cause for the null the types now rule out:
     - `W/fill.ts`: `'the fill stopped before reaching this file'` and the three `'Storage did not answer'`;
     - `W/refresh.ts`: `'the catch-up did not finish'`.
   - Real causes include `locked`, `server` and a refused root, so "Storage did not answer" was false whenever it showed.
   - The narrowers in `G/contract.test.ts` reject `status: 'stopped', stopped: null`, and a run that is not stopped carrying a refusal. Test fixtures that spread `Partial<Fill>` across the two fields (`W/StatusBar.test.tsx`, `fill.test.ts`, `retry.test.ts`, `dismissed.test.ts`, `useWork.test.tsx`, `newFolder.test.ts`) build one variant or the other.
2. **A displaced run is a stopped run, and its type says so.**
   - `FillDto.displaced` and `FreezeDto.displaced` (`S/routes/work.rs`) are full run DTOs, even though every element is stopped and has empty `waiting` / `discarded` / `displaced`. The comment in `S/latest.rs` says the same.
   - Give each its own DTO: the run without its queue lists, with status fixed to stopped and `stopped` non-null. Mirror it with TS types in `G/work.ts`.
   - Readers use the fields the displaced run still has, so keep them:
     - `declined[]`, `total` and `done` for a fill, which `rowFill` draws from;
     - `packs`, `entries`, `findings` and `step` for a freeze.
   - `RefusalDto` stays shared. The same failure keeps one wording whether it sits on the current run or a displaced one.
   - Retype the readers that take `Fill | Freeze | Sync` so a displaced run is accepted: `rowFill`, `fillLine`, `freezeLine`, `retryable`, `stoppedLine`, `stoppedBooksLine`, `shownRuns`, `newFolder.ts`.
   - The wire change is that the three always-empty lists stop being sent on displaced elements. Update `tests/routes.rs` (the displaced cases near `a_fill_storage_stopped_is_still_named_once_the_next_folder_runs` and its freeze counterpart, which assert `displaced[0]["waiting"]` is empty) and regenerate `G/contract/work.json` with `COFFRET_WRITE_CONTRACT=1`.
   - The fixture should also carry a displaced fill that has `declined` entries, so that path is exercised.
3. **Refusals that arrive inside an answer go through the same narrowing as refusals that arrive instead of one.**
   - Today the request path goes through `refusalOf` in `G/refusal.ts`. It maps an unknown kind to `unrecognized` and an unknown reason or surfaced value to `null`.
   - The work answer is cast instead: `askedForJson<Work>` in `getWork` / `startSync` / `startFill` / `startFreeze`. So is the upload's per-part list (`askedForJson<Upload>`, `G/upload.ts`). Their `stopped`, `declined[]`, catalog `stopped`, `displaced[]` and `refused[]` reach `W/retry.ts`'s comparisons as raw strings that merely claim the type.
   - Decode those answers with a narrowing function built from the same `kindOf` / `reasonOf` / `surfacedOf`, so an unknown value lands where it lands on the request path. Findings' `reason` / `surfaced` (`G/work.ts`) go through the matching narrowing as well.
   - While at it, drop the second definition of one refusal: TS `Refused` (the interface on answers) and `Refusal` (the class thrown on the request path) describe the same four fields. Let answers carry one shape, and name it once.
   - Note that the contract-test narrower `refused()` in `G/contract.test.ts` throws on unknown literals; the runtime narrowing must not.
   - Add gateway unit tests that feed an answer with an unknown `error` / `reason` and see it narrowed.
4. **The unmapped sentence is stated once.**
   - `W/unmapped.ts` copies the server's `no_folder_here` sentence ("no folder on this device holds this part of the Library") as a literal. It has to, because clicking an unmapped folder makes no request. The server also builds the same sentence twice: `ApiError::no_folder_here` and the `FetchError::UnmappedEntryPath` arm in `S/api_error/from_error.rs`.
   - On the server, have both use one constant.
   - On the frontend, export the sentence as a constant from `@coffret/api` (its `exports` is `"."` only, so export it from the package entry). `G/contract.test.ts` should check that constant against `G/contract/refusals.json`, and `W/unmapped.ts` should import it.
   - The frontend embeds the clause mid-sentence, so keep casing and punctuation compatible.
5. **Wire kinds are spelled only in `api_error`.**
   - `S/reported.rs` spells the kinds `"server"` (in `unfinished()`) and `"storage"` (in `gave_up()`) by hand.
   - Take them from `api_error`, either as kind constants it exports or as constructors that return the `Reported` directly. The sentences stay those two functions' own, as their docs explain.
6. **Each 409 and each `unverified` is built in one place.**
   - The `declined` / `refused_placement` struct literals are repeated across `S/api_error/mod.rs` (the `surfaced` / `reserved` constructors, `no_folder_here`, `pack_resident`, `declined_as`) and `S/api_error/refused_root.rs`. Build each kind through one private constructor per kind, with the cause optional.
   - `"unverified"` is spelled four times in `S/api_error/from_error.rs`, two of them with the same message ("what reached Storage is not the content this device sent"). Give it one constructor, with the message as its argument, and reuse one constant for the repeated message.
7. **The refusal fixture covers every combination a browser branches on.**
   - `every_refusal()` in `S/api_error/contract.rs` says it holds "one per combination a browser branches on, in the order the explorer's `RefusalKind` names the kinds". Since `no_folder_here` became `refused_placement`, it no longer holds a fetch's `declined` + `unmapped`, which `GET /api/file` does return (`tests/routes.rs`, the unmapped-fetch case).
   - Add `fetch(FetchError::UnmappedEntryPath { .. })` as that representative, and put the entries back in `RefusalKind`'s order. Regenerate `G/contract/refusals.json`.

Guard: no behaviour change a person could see beyond the fallbacks' removal. The same refusals are sent in the same cases with the same messages. The wire changes only as item 2 describes.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `W/fill.ts` and `W/refresh.ts` no longer invent a cause for a stop ("Storage did not answer", "the fill stopped before reaching this file", "the catch-up did not finish" are gone; grep gates), and the contract narrowers reject a stopped run without a refusal and a running one with one (`make check`)
- [x] The work answer and the upload answer are no longer cast with `askedForJson<Work>` / `askedForJson<Upload>` (grep gate), and gateway unit tests show an unknown kind or reason inside an answer narrowed (`make check`)
- [x] `W/unmapped.ts` no longer spells the unmapped sentence (grep gate), and the contract test checks the exported constant against `refusals.json` (`make check`)
- [x] `S/reported.rs` spells no wire kind (grep gate); `S/api_error/from_error.rs` spells no `"unverified"` (grep gate); at most one struct literal per 409 kind remains under `S/api_error/` (count gate)
- [x] `refusals.json` carries `unmapped` under both kinds (count gate), and the displaced-run DTOs and their TS types exist with the route tests updated (`make check`)
- [x] `make check` passes
