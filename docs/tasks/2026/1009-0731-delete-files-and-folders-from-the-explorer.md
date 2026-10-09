---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1009-0731-delete-files-and-folders-from-the-explorer
created_at: 2026-10-08T22:31:08Z
updated_at: 2026-10-09T01:39:15Z
---

# feat(explorer): delete files and folders from the explorer, after showing what leaves the Library and what is rebuilt

## Overview

The usecase can now delete Entries (`delete_entries`, `preview_delete` in
`coffret-usecase/src/delete/`): one-file Containers and fully deleted Packs
are removed, Packs that keep other Entries are rebuilt by read-modify-replace
(PK-9, PK-10), and a partly deleted key-lost Pack is refused (KL-17). Nothing
in the server or the explorer reaches it yet, so a file or folder brought in
by mistake can still only be "removed" by deleting objects in the Storage
provider's own UI, which breaks the Library.

**1. Server routes.** Following the freeze pair (`GET` / `POST /api/freeze`),
add `GET /api/delete` (preview) and `POST /api/delete` (arm). Each names what
to delete — a folder (`?path=<folder>`) or a set of Entry Paths (in the body).
The preview answers what `preview_delete` answers: how many Entries and bytes
leave the Library, how many Containers are removed outright, how many Packs
are rebuilt and the bytes read and written to rebuild them, and which Packs
would be refused and why. The device's key-lost set comes from the committed
Keyring the server already reads. `POST` arms a delete run on its own worker
(like freeze: one at a time, queued, reported through `GET /api/work` with its
own progress and outcome), so a long rebuild does not block the request. Both
are refused where a freeze is refused for the same reasons (locked Library;
nothing current under the path) and the Library root as a whole is refused.

**2. Explorer.** Give a file row and a folder an action "Delete…" (and a
multi-selection of files, if the explorer has selection). It asks the preview,
then shows a confirmation stating:
- what leaves the Library: "12 files, 340 MB will be removed from the Library";
- what a rebuild costs where Packs keep other files: "2 books packed with
  others will be rebuilt — reads 1.8 GB, writes 1.5 GB";
- anything refused, with the reason;
- that this cannot be undone from the explorer.
"Delete" then arms the run and the explorer follows it (progress, a notice
when done naming what was deleted and anything refused). Cancel arms nothing.

**3. Conflicts read as conflicts.** A commit refused because the Library moved
underneath (`CommitError::RemovalNotCurrent`, `EntryPathCollision`,
`ConflictLimitReached`) currently reaches the explorer as a generic server
failure ("the server could not answer"). Give these a refusal the explorer can
act on — a reason saying another device changed the Library meanwhile and that
running it again plans from the current state — and use it for delete,
freeze and sync alike. Keep the existing contract shape (`kind` / `reason`)
and update the contract fixtures and the frontend types.

**4. Local files.** What happens to the deleted files on devices that hold them
is a separate change; here, the confirmation says only what is removed from
the Library. Do not touch local files.

Out of scope: restoring a deletion; the local-file handling above; repack /
compaction.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] `GET /api/delete` answers the preview for a folder and for a set of Entry Paths, and its counts and bytes match what the run then does (route test)
- [x] `POST /api/delete` arms a delete run that commits through the usecase, is reported in `GET /api/work` with progress and outcome, and queues behind a running one (route tests)
- [x] Both routes are refused for a locked Library, the Library root, and a path with nothing current (route tests)
- [x] The explorer offers "Delete…" on files and folders, shows the confirmation with the removed amount, the rebuild cost, refusals and the not-undoable note, and arms only on confirm (unit and component tests)
- [x] A commit refused because the Library moved underneath reaches the explorer as an actionable conflict refusal for delete, freeze and sync (route and contract tests)
- [x] `make check` passes

### Before merge (verified outside the check command)
- [ ] Needs a person: under `make desktop-dev`, deleting a one-by-one file and a volume inside a packed book shows the confirmation, and afterwards the explorer no longer lists them while the rest of the book is still readable
