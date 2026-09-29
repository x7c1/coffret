---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 2
retries_remaining: 1
check_command: 'make check && make e2e-it && ! git grep -qE "api/lock|lockServer|askToLock|stillOpen|LockedDto" -- backend frontend && ! git grep -qE "^- \*\*DK-3\." -- docs/spec && git grep -qE "^- \*\*DK-4\." -- docs/spec && git grep -q "lockLanded" -- frontend && ! git ls-files frontend/packages/apps/e2e | grep -q "lock\.spec\.ts$" && ! git grep -qE "either because" -- backend/crates/apps/coffret-server/src/api_error frontend/packages/gateway/api/src'
assignee: null
branch: task/0929-0447-drop-the-explicit-lock-and-keep-the-idle-lock
created_at: 2026-09-29T04:47:35Z
updated_at: 2026-09-29T05:17:56Z
---

# feat!: drop the explicit lock, and keep the idle lock

## Overview

The server throws its keys away two ways (spec: DK-3, DK-4): the explorer's `lock` button calls `POST /api/lock`, and the server locks itself after the idle interval. Either way the only unlock is starting the server again with the Passphrase, so the button does what stopping the server does, and a stopped server holds no keys either. The button is also a source of its own trouble: when the reader is open and the request does not reach the server, the sentence saying the Library is still open lands in the notice area under the reader and is cleared by the move that closes the reader, so it is never seen. For a person using coffret alone on their own desktop, the button is complexity with nothing to pay for it. Remove the explicit lock; keep the idle lock, which is what covers a Library left open.

**Remove:**

- The server's `POST /api/lock`: the route in `backend/crates/apps/coffret-server/src/router.rs`, `routes/lock.rs` and `LockedDto`, and the doc comments that describe it (`lib.rs`, `lock/mod.rs`, `routes/mod.rs`, `state.rs`, `lock/custody.rs`, `lock/lock_when_idle.rs`, and `coffret-device/src/lib.rs`, which cites DK-3).
- Its route tests and fixtures: `tests/routes/lock.rs` cases that exercise the explicit lock, the `POST /api/lock` entries in `tests/routes/contract.rs` and `tests/routes/fences.rs`, and the matching golden fixture under `frontend/packages/gateway/api/src/contract/`. Where a test uses `POST /api/lock` only to put the server into its locked state in order to test something else — a keyed route refused while locked, the activity answer reporting `locked` — keep the test and reach that state another way (the idle interval, or the state the server holds, as the idle-lock tests already do).
- The explorer's side: the `lock` button and its `onLock` / `locking` props in `StatusBar.tsx`, the wiring in `App.tsx`, `askToLock` and `stillOpen` in `lock.ts` with their tests, and `lockServer` in `@coffret/api`.
- The e2e journey `frontend/packages/apps/e2e/journeys/08-lock.spec.ts`.
- Every sentence that names the explicit lock as a way a server comes to be locked — above all the refusal a keyed route answers with while the server is locked (`ApiError::locked()` in `api_error/admission.rs`, pinned by the golden fixture `frontend/packages/gateway/api/src/contract/refusals.json` and by `refusal.test.ts`), which says why the server is locked and now names the idle interval alone.

**Keep:** the idle lock (DK-4) and everything it needs — `lock_when_idle`, the custody cell being emptied, the activity answer's `locked` / `unlocked` state, and `lockLanded` in `lock.ts`, through which a reader left open gives up its decrypted pages when the idle lock lands. Where `lock.ts`'s header describes the two roads a lock arrives by, it now describes one.

**Spec and concepts.** Delete the DK-3 entry from `docs/spec/device-key-custody/README.md`. Per `docs/spec/README.md`, IDs are never renumbered or reused, so DK-3 stays a gap. Update what refers to it: DK-1's note that "DK-3 and DK-4 stand as written", DK-9's "Past the explicit lock (DK-3)", the Device Key Custody row of the Mechanisms table ("explicit and idle locking"), and the concepts — `docs/concepts/master-key/README.md` (the *lock* collocation says "explicitly or after an idle interval"), `docs/concepts/library/README.md` and `docs/concepts/passphrase/README.md`. State what is true after the change: a device locks after the idle interval, and stopping the server ends its hold on the keys. `make spec-rule-ids` refuses any citation of DK-3 left behind.

Out of scope: the Japanese copies of the concepts are kept outside this repository and are synced separately.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Nothing under `backend/` or `frontend/` names `POST /api/lock`, `lockServer`, `askToLock`, `stillOpen` or `LockedDto` (grep gate)
- [x] The register has no DK-3 entry and still has DK-4, and no citation of DK-3 is left anywhere `spec-rule-ids` looks (grep gates, and `make check` runs `spec-rule-ids`)
- [x] The explorer still gives up a reader's pages when the idle lock lands: `lockLanded` stays in `frontend/` with its tests (grep gate, and `make check` runs the frontend tests)
- [x] The refusal a locked server answers with names the idle interval alone, and the fixture and the test that pin it agree (grep gate over `api_error` and `@coffret/api`'s sources, and `make check` runs the golden contract test)
- [x] The e2e journey for the explicit lock is gone and the remaining journeys pass (`git ls-files` gate, and `make e2e-it`)
- [x] `make check` passes
