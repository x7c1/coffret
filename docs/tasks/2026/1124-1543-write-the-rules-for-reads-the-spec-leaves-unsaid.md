---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, spec-alignment, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/1124-1543-write-the-rules-for-reads-the-spec-leaves-unsaid
created_at: 2026-11-24T06:42:28Z
updated_at: 2026-11-24T08:42:50Z
---

# docs(spec): write down what the server already does where no rule says it, and drop the delete dialog's refusal rows

## Overview

Several things the code does today are stated by no rule, are stated twice, or are stated in words that the code does not match. Apply one rule to every site below: **the spec and the concept docs say what the code does, once.** Every rule added or reworded here describes present behaviour; none of them changes it. Read the code and its tests before writing each rule, and write what they do — where the description below disagrees with the code, the code wins, and say so in the PR description. Follow the form of the neighbouring rules (numbering, the terms the spec already uses, the way tests cite a rule), and cite each new or reworded rule from the code or the test that upholds it, the way the surrounding code cites rules.

- **The deletion's estimate (`GET /api/delete`).** No rule defines it. It is counted from the device's own catalog alone and never touches Storage — no catch-up, no read of the Keyring; a Pack whose key is lost is counted as one to be rebuilt, and the deletion itself is what refuses it (PK-10); what the estimate names as refused is always empty. Write it into `docs/spec/loopback-access/README.md` after the pattern of LA-30, which defines the estimate of a move.
- **The delete dialog's refusal rows.** `frontend/packages/apps/web/src/DeleteConfirm.tsx` (around line 71) still draws a row for each refusal the estimate names, and since the estimate never names one, the rows are never drawn. Remove the rows, and remove the field from the estimate's answer and its types on both sides (server DTO, web gateway/domain types, fixtures and tests) if nothing else reads it. If something does, keep the field, remove only the rows, and say so. This is the one code change here, and it changes nothing a user can see.
- **Who the desktop app hears "session ended" from.** `frontend/packages/apps/desktop/src/renewal.ts` (`sessionEndedListener`, with tests) accepts it only from the explorer's own window and from the explorer's origin. LA-21 does not say so. Add to LA-21: "Only the explorer's own window, from the explorer's origin, is heard saying so; anything else saying it is let be." (adjust the wording to fit the rule).
- **One duty stated twice.** That a per-Library token cache left in a Library's state is neither read nor removed is stated both under SA-8 (cited by `backend/crates/apps/coffret-device/src/account_tests.rs`, `//! SA-8:`) and in KD-10 (`docs/spec/key-derivation/README.md`). Keep the statement in the rule that owns it and make the other a reference to it; keep the test's citation pointing at a rule that says what the test checks.
- **The Index's per-folder totals.** The Mental Model of `docs/concepts/index/README.md` has no layer for the per-folder totals the Index keeps (`folder_totals`, `catalog_counts`), recounted in the same transaction as each write. Add it in the concept doc's own vocabulary, not the table names alone.
- **`GET /api/folders`, and the nulls in a folder's listing.** No rule says what `GET /api/folders` answers; LA-45 only says what it costs. LA-44 does not say when a row's `mtime` and deposit time are null. Write both.
- **LA-45 and the reservations.** LA-45 says the rows are looked up by key, while `/api/list` reads every reservation of the device (`backend/crates/apps/coffret-server/src/routes/list.rs`, `reserved_paths()`). The reservations are those of operations in flight, so their number does not grow with the Library. Do not narrow the read; reword LA-45 so that it is true, saying what the reservations are bounded by.
- **What the reader's reads cost.** No rule says how much `/api/beside`, the fill's "is anything missing", and the lookup of held parcels read of the Index. Each now reads only what it touches (a lookup by id or key, a range ended by the first row, or a test for existence). State it beside LA-32, in the form LA-45 gives the listing's cost.
- **The filter's name stops at the limit in the explorer.** The explorer stops the name it filters by at the 1,000 characters the server keeps (LA-43), counted as the server counts them (Unicode scalar values: `chars().count()` on the server, code points in `frontend/packages/apps/web/src/filter.ts`, `nameWithinLimit`), wherever the name is typed or pasted, so it never asks to keep a name the server refuses. LA-43 only says the server refuses a longer name. Add the explorer's side to LA-43, and say there what "characters" counts. The tests that name LA-43 for this already exist (`FilterField.test.tsx`, `useKeptFilter.test.tsx`).

Do not change behaviour, and do not add a rule that asks for behaviour the code does not have. Do not write a rule for the explorer asking again after a load that got no response; that is left unstated on purpose. Change nothing under `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] Each rule added or reworded is cited by the code or test that upholds it, in the form the repository's checks expect
- [x] `DeleteConfirm.tsx` draws no refusal rows, and no field of the estimate is left that nothing reads (or the PR description says what still reads it)
- [x] The duty about the left-behind per-Library token cache is stated in one rule, and the other place refers to it

### Before merge (verified outside the check command)

- [x] The PR description lists each rule added or reworded with the code it was read from, and each place where the code differed from this task's description
