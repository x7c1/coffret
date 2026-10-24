---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rn "let_go" backend/crates/domain/coffret-usecase/src/fetch/parcel_read/ && grep -rqF "reconnect" docs/concepts/storage/'
assignee: null
branch: task/1024-1430-bring-seven-concept-and-spec-passages-up-to-the-code
created_at: 2026-10-24T05:30:00Z
updated_at: 2026-10-24T10:46:46Z
---

# docs: bring concept, spec and comment passages up to the code, and keep "let go" and "reconnect" to one meaning each

## Overview

Review of the last few changes found concept and spec text that no longer
says what the code does, and two words used for two things each. Concept
readers reason from the vocabulary being consistent, so each is fixed here.

1. **Index concept, Collocations, "complete"** (`docs/concepts/index/README.md`)
   does not cover the completion right after this device's own commit when
   another process over the same Index applied the record before the refresh.
   Add that case.
2. **Index concept, Domain Rules, "refuses a record at or below the head"**
   does not say that a commit's refresh meeting that refusal records its
   device-local records alone, in one operation, and completes. Add it as a
   sub-bullet (the code is `backend/crates/domain/coffret-usecase/src/commit/run.rs`,
   `AlreadyApplied`).
3. **OC-8** (`docs/spec/orphan-cleanup/README.md`, the scratch sub-bullet)
   does not say that a writer whose filesystem gives no lock refuses the write
   the same way, removing the scratch it made first. A scratch another writer
   holds the lock on (a sync) is the same case.
4. **Library concept** (`docs/concepts/library/README.md`): no statement that
   a mapped folder is expected on a local disk and that on a filesystem
   giving no file lock every placement is refused, with "on a local disk" as
   the remedy (spec: OC-8). Add a sub-bullet to the scratch Domain Rule and an
   example among the Collocations' refusals.
5. **Library concept "drop" and Pack concept Collocations**: "a freeze of
   exactly the files dropped" is no longer exact, because a continued drop's
   freeze also takes what the cut-off drop received; and the Pack concept has
   no "continue (a drop in progress, by dropping its folder again)". Fix both,
   citing PK-17.
6. **"reconnect" has two meanings.** SA-10 (`docs/spec/storage-authorization/`)
   uses it for renewing a grant from the explorer; the Library and Mapping
   concepts (`docs/concepts/library/README.md` around "a root to reconnect",
   `docs/concepts/mapping/README.md` likewise) use it for plugging a disk back
   in. Decided: "reconnect" means renewing an account's grant on this device;
   define it so in the Storage concept's Collocations beside
   `renew (an account's grant on this device)`, and reword the two disk
   passages with another verb (for example "a root to bring back" / "make
   available again" — keep EP-12's terms). Grep the rest of `docs/` and code
   comments for the disk sense and reword those too.
7. **"let go" used for unheld parcels.** The Index concept keeps "let go" for
   held parcels. `backend/crates/domain/coffret-usecase/src/fetch/parcel_read/`
   has a module and function `let_go` / `let_go_departed` about parcels no row
   holds. Rename the module, functions and comments there to a word that does
   not collide (follow the wording `unheld_parcel.rs` already uses), and leave
   `let_go_parcels` alone where it is about held parcels.

8. **Library concept, the rule on system files** (`docs/concepts/library/README.md`,
   the Domain Rule on what an operating system keeps, EP-17): it says a sync
   or a freeze says nothing of them, but not that a freeze's preview counts
   them among what it leaves as it is ("Pack this folder…" lists them under
   "left as they are"); and its cost sentence names only a name beginning
   `._`, where EP-17's cost is any file or folder of the person's own
   carrying one of the listed names. Add the first, and widen the second to
   match EP-17.

9. **A comment in `delete/mapped_copies.rs`** (`backend/crates/domain/coffret-usecase/src/delete/mapped_copies.rs`,
   the branch where `mapped_source` answers `None`, comment beginning "Gone
   from the disk as well as from the Library"): that branch is now also
   taken for a path at a name EP-17 leaves out, where the device's file is
   left in place and only the row goes. Say so in the comment, citing EP-17.

The Japanese copies of the concept documents are kept outside this
repository and are regenerated separately, not in this PR.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Items 1–5, 8 and 9 are in the named documents, each citing the rule it rests on.
- [x] "reconnect" is defined in the Storage concept (gate appended) and no
      concept or spec passage uses it for a disk coming back.
- [x] No `let_go` remains under `fetch/parcel_read/` (gate appended), and the
      workspace builds.
