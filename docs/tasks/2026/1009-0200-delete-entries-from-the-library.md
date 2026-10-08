---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1009-0200-delete-entries-from-the-library
created_at: 2026-10-08T17:00:54Z
updated_at: 2026-10-08T19:34:41Z
---

# feat(usecase): delete Entries from the Library, rebuilding the Packs that keep others by read-modify-replace

## Overview

Nothing can remove an Entry from the Library today except replacing it: the
only removals a batch carries are Containers that `sync` or `freeze` replace
(`coffret-usecase/src/commit/prepared_batch.rs`, `spooled_container.rs`).
The spec already defines deletion — PK-9 (deleting a folder removes Packs
whose Entries are all deleted and replaces Packs that keep other Entries by
read-modify-replace), PK-10 (read-modify-replace reads and verifies every
Entry of the old Pack, carries the kept ones forward, omits the deleted ones),
CP-14 (a removed Container ID never comes back), KL-17 (a key-lost Container
leaves only by a genuine committed removal) — but no code implements it, and
read-modify-replace does not exist yet. This change adds the usecase; the
server route and the explorer come in a separate change.

**1. A delete run.** Add a usecase (alongside `sync`, `freeze`, `fetch`) that
takes a set of Entry Paths and/or a folder prefix to delete and commits one
Journal batch:
- an Entry in a one-file Container: the Container goes to removals;
- a Pack all of whose current Entries are deleted: the Pack goes to removals;
- a Pack that also keeps Entries: read-modify-replace (below) produces a new
  Pack holding only the kept Entries; the old Pack goes to removals and the new
  one to additions, in the same batch (PK-9, PK-12's batch shape, CP-1, CP-14).
After the commit, removed objects are trashed through the existing
`after_commit::trash_removals` path, which already keeps the commit valid and
reports a failed trash.

**2. Read-modify-replace (PK-10).** Fetch the old Pack, verify every Entry
(the existing chunk authentication and Entry hash checks), and write a new
Pack with the kept Entries in their original order and with their recorded
metadata (Entry Path, `original_mtime` / `original_btime`, hash), encrypted
under a new Container Key, through the same writer `freeze` uses
(`ContainerWriter`, `EntryPlan`). If any kept Entry cannot be read or does not
verify, commit nothing for that Pack and report why; never write a
replacement missing a kept Entry. Build it so `update` (PK-12) can reuse it
later, but do not implement `update` here.

**3. Key-lost Packs (KL-17, PK-10).** A Pack whose key is lost cannot be
read. Deleting all of its Entries is allowed (it simply goes to removals).
Deleting only some of them would need a read-modify-replace that cannot
happen: refuse that deletion for that Pack with a reason naming the Pack's
kept Entries, and commit nothing for it. Do not invent a way to keep part of an
unreadable Pack.

**4. A preview.** Like `preview_freeze`, add a read-only preview of a delete:
how many Entries and bytes would leave the Library, how many Containers would
be removed outright, how many Packs would be rebuilt and the bytes that
rebuild would read and write, and any Pack that would be refused (key-lost,
partly deleted). It reads the catalog only: no Storage, no key, no writes.

**5. Spec.** Make PK-9 cover deleting individual Entries as well as a folder
if it does not already, state the key-lost refusal under PK-10 or KL-17, and
note that deletion is not undoable in the Library (CP-14) and that Storage's
trash keeps the removed ciphertext only until it is purged — restoring from it
does not restore membership or keys. Update the Library and Pack concepts'
collocations / rules for `delete` if they describe it differently.

Out of scope: the server route and the explorer UI; what happens to local files
on devices that hold a deleted Entry (a separate change); `update`; repack /
compaction.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] Deleting an Entry in a one-file Container commits one batch removing that Container, with a conformance test
- [x] Deleting all Entries of a Pack removes the Pack; deleting some rebuilds it with exactly the kept Entries (paths, times, hashes, order) under a new Container ID, old Pack in removals and new in additions in one batch — conformance tests including a folder spanning several Packs
- [x] A kept Entry that fails to verify during read-modify-replace commits nothing for that Pack and reports it (test)
- [x] A key-lost Pack can be deleted whole, and a partial deletion of it is refused with a reason (tests)
- [x] Removed objects are trashed after commit; a failed trash leaves the commit valid and reported (existing path, tested for delete)
- [x] The preview's counts and byte totals match what the delete run then does, and the preview touches no Storage and needs no key (tests)
- [x] The spec states deletion of Entries, the key-lost refusal and non-undoability, and `make spec-citations spec-rule-ids` passes
- [x] `make check` passes
