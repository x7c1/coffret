---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && ! grep -rqi --exclude-dir=node_modules --exclude-dir=target --exclude-dir=dist --exclude-dir=dist-types "reconcil" backend/crates/ frontend/packages/ docs/concepts/ docs/spec/ scripts/ && grep -qw "settling" frontend/packages/gateway/api/src/activity.ts && ! grep -qF "uploading" frontend/packages/gateway/api/src/list.ts && grep -qF "\"added\"" backend/crates/apps/coffret-server/src/routes/list.rs && ! grep -rqF "\"state\": \"uploading\"" frontend/packages/gateway/api/src/contract/ && ! grep -qF "uploading" backend/crates/apps/coffret-server/src/routes/list.rs && ! grep -qF ".state == \"uploading\"" scripts/e2e-it.sh && grep -rqF "not in Library" frontend/packages/apps/web/src/ && grep -qF "left the Library" docs/concepts/library/README.md && ! grep -qF "entry table is settled" docs/spec/pack-construction/README.md && ! grep -qF "entry table settled" docs/spec/README.md && ! grep -qF "settles the table first" docs/concepts/container/README.md'
assignee: null
branch: task/0927-1347-call-the-settle-stage-and-the-added-row-by-their-registered-names
created_at: 2026-09-27T13:47:40Z
updated_at: 2026-09-27T14:15:42Z
---

# refactor: call the settle stage and the added row by their registered names

## Overview

Two things the concept documents already name are still called something else in the code and on the wire between the server and the explorer, and one registered verb is used as a plain verb in the spec. This change renames them. The only behaviour a person can see is the label on a file row that is in a mapped folder and not in the Library.

1. **The first stage of a sync is a settle.** The Library concept registers *settle* ("what an interrupted run left behind, before this one scans"), and the explorer already shows the stage as "settling what an interrupted run left". The code calls it *reconcile*: `backend/crates/domain/coffret-usecase/src/sync/reconcile.rs` (whose module doc says the name predates the split of the two acts "reconcile" once covered), `sync/reconciled.rs` (`Reconciled`, re-exported from `coffret-device`), `SyncOutcome::reconciled`, `Phase::Reconciling`, the device's and server's findings and progress, the CLI's progress text, and the wire phase value `"reconciling"` (`coffret-server/src/routes/activity.rs`, `frontend/packages/gateway/api/src/activity.ts`, the activity contract fixture, `apps/web/src/fill.ts`). Rename all of it to the settle family — `settle.rs`, `Settled`, `SyncOutcome::settled`, `Phase::Settling`, wire value `"settling"` — together with the tests and conformance cases that use the old words. The one other use, `conformance/conditional_create.rs` ("refresh the head, reconcile, and retry"), is CP-4's rebase of a losing writer's batch; say *rebase* there.
2. **A row that is in the folder and not in the Library is `added`.** The listing's `state` field (`coffret-server/src/routes/list.rs`, `frontend/packages/gateway/api/src/list.ts` `EntryState`) says `uploading` for a file standing in a mapped folder that the Library holds no Entry for. Nothing is being uploaded when that is said: the row may be a file just added, which the next sync will carry in, or one whose Entry left the Library when another device removed its Container and which stays on disk (`coffret-device/src/add/added_file.rs`). The device layer already calls the row `AddedFile`, and the Library concept calls the state *added*. The same spelling `uploading` is also the sync phase that really does send to Storage, which stays as it is.
   - Change the wire value to `added`, in the server, the TypeScript type, the contract fixture (`gateway/api/src/contract/answers.json`), the route tests, `scripts/e2e-it.sh`, and the explorer (`apps/web/src/fill.ts`, `FileList.tsx`, `FolderTree.tsx`, `App.tsx`, `theme.ts`, their tests) and its journeys (`apps/e2e/journeys/03-drop.spec.ts`, `07-freeze.spec.ts`).
   - The explorer's chip stops printing the wire value. It maps states to words the way the phase line already does (`DOING` in `fill.ts`), and the word for this state is **not in Library**. `present` and `remote` may keep their own words.
   - Widen the Library concept's *added* bullet (`docs/concepts/library/README.md`, the EP-10 sub-bullet "A file merely **added** …") so it covers the second way into the state: a file whose Entry left the Library while the file stayed on disk. It is still not materialized, and a scan reports it as new.
3. **`settle` as a plain verb in the spec and the Container concept.** PK-18 in `docs/spec/pack-construction/README.md` ("A Pack's entry table is settled before any of its content is written"), its row in the spec index (`docs/spec/README.md`, "the entry table settled before content is written"), and the Container concept (`docs/concepts/container/README.md`, "settles the table first") use *settle* for fixing a table, which the Library concept's survey rule now says as *fixes*. Use the same word in all three.

Out of scope: the sync phase `uploading` (it names a real upload), and any compatibility with an explorer or server built before this change — both halves ship together.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] No *reconcil…* remains under `backend/crates/`, `frontend/packages/`, `docs/concepts/`, `docs/spec/` or `scripts/` (grep gate), and the activity wire type has `settling` (grep gate)
- [x] The listing's state is `added` on the wire: not `uploading` in `routes/list.rs`, `list.ts`, the contract fixture or `scripts/e2e-it.sh`, and the server emits `"added"` (grep gates)
- [x] The explorer labels the state "not in Library" (grep gate), and the drop and freeze journeys pass against it (`make e2e-it`)
- [x] The Library concept's *added* covers a file whose Entry left the Library (grep gate on "left the Library")
- [x] PK-18, its index row and the Container concept no longer use *settle* for fixing a table (grep gates)
- [x] `make check` passes, including the golden-fixture contract tests between the server and the explorer
