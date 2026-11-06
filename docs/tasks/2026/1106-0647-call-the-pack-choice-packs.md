---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rn --exclude=deleteEntries.ts "One Pack" frontend/packages/apps/web/src frontend/packages/apps/e2e/journeys'
assignee: null
branch: task/1106-0647-call-the-pack-choice-packs
created_at: 2026-11-06T06:47:56Z
updated_at: 2026-11-06T06:54:45Z
---

# fix(web): call the drop's Pack choice Packs

## Overview

A drop holding a folder asks how the folder is stored ("Store the folder as",
`DropConfirm.tsx`), offering **One Pack** or **Separate files**
(`STORE_CHOICES` in `frontend/packages/apps/web/src/packChoice.ts`). "One
Pack" is not what happens: segmentation cuts Packs at the size target
(`docs/spec/pack-construction/README.md`, PK-3 and PK-5), so a folder over
1 GiB is stored as several Packs.

Rename the choice so that it reads on from "Store the folder as" and stays
true at any size:

- label **Packs** (was "One Pack");
- note **packed together with the rest of the drop, best for books and
  albums** (was "kept and fetched together, best for a book or an album").
  The note gives no size figure: the size target is a pack-policy parameter
  (PK-5), not something the explorer promises.
- **Separate files** and its note stay as they are.

Bring every place that names this choice "One Pack" in line, in code, comments,
test names and assertions, and the e2e journeys: for example
`DropConfirm.tsx`, `App.tsx`, `dropTarget.ts` (`PACKED_AFTER`'s message),
`packChoice.ts`, their tests, and `frontend/packages/apps/e2e/journeys/`
(`books.ts`'s `AS_ONE_PACK`, `07-freeze.spec.ts`). A user-facing message that
names the choice (such as `PACKED_AFTER`) uses the new label.

Guard: "one Pack" where it states a fact about how many Packs something is —
"a book that is one Pack", "One Pack holding two Entries", "One Pack the
deletion is refused for" and the like, in `work.ts`, `deleteEntries.ts`,
`fill.ts`, the backend and the format tests — is not the choice and is not
changed. Generated `dist-types/` files are not edited by hand.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The drop confirmation offers **Packs** with the note "packed together
      with the rest of the drop, best for books and albums", chosen by
      default, beside **Separate files**, under the web unit tests.
- [x] No "One Pack" naming the choice remains under
      `frontend/packages/apps/web/src` or `frontend/packages/apps/e2e/journeys`
      (`deleteEntries.ts`, whose "One Pack the deletion is refused for" is the
      guarded fact, aside).
- [x] `make check` passes.
