---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! git grep -n -E "MovedToTrash|moved_to_trash|movedToTrash|move_departed_to_trash" -- backend frontend docs/spec docs/concepts && ! git grep -n -E "^- move \(" -- docs/concepts'
assignee: null
branch: task/1017-1600-call-sending-a-departed-file-to-the-trash-discard
created_at: 2026-10-17T07:00:02Z
updated_at: 2026-10-17T07:28:19Z
---

# refactor: call sending a departed local file to the desktop's trash "discard", and leave "move" unassigned

## Overview

The Library concept's Collocations give the everyday verb **move** a narrow meaning: "move (a departed file to the desktop's trash, during a sync, or right after the commit of the deletion this device made) — never *trash*, which is Storage's verb". *trash* stays Storage's verb for Storage Objects, and that part is right. But pinning *move* to the desktop trash leaves no ordinary word for what the planned rename does: the rename changes an Entry's path in the catalog, and a device that holds the Entry's file follows by moving that file on disk, which will be called **relocate**. The explorer will later offer "Move to…", whose effect is a rename. With *move* reserved for the trash, all of these read as one verb meaning three things.

Give the desktop-trash act its own verb, **discard** ("discard a departed file into the desktop's trash"), and stop assigning *move* to any concept: a rename is *rename* (including a change of folder), the local follow-up is *relocate* (introduced by the rename work), and "Move to…" remains user-interface wording whose effect is a rename.

Apply the rule everywhere the desktop-trash act is named:

- Concepts: the Library concept's Collocations (`move` → `discard`, the `remove` entry's "a run's moves to the trash" wording, and every other sentence in `docs/concepts/` that says a departed or deleted Entry's local file is *moved* to the desktop's trash).
- Spec: EP-15 / EP-16 and any rule whose prose uses *move* for this act.
- Rust: identifiers, types, modules and functions (`departure/move_departed_to_trash.rs`, `FindingReason::MovedToTrash` and its siblings in `coffret-device` and `coffret-server`, `delete/mapped_copies.rs`, `sync/disposal.rs`, …), log fields, and the human-readable finding text (`moved <path>: …` becomes `discarded <path>: …`).
- The server → explorer wire value `MovedToTrash` and the TypeScript that reads it (`frontend/packages/apps/web/src/StatusBar.tsx` and wherever the reason is typed).
- Tests and fixtures that assert any of the above.

Wording that is not this act stays: *trash* for Storage Objects, a file system's own `rename(2)`, and *move* in prose that is not naming a coffret concept (e.g. "a person moves a file on disk"). None of this is stored in Storage, so no Library is affected.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The Library concept's Collocations list `discard` for the desktop-trash act and no `move` entry (grep gate).
- [x] No `MovedToTrash` / `moved_to_trash` / `movedToTrash` / `move_departed_to_trash` remains in `backend`, `frontend`, `docs/spec` or `docs/concepts` (grep gate), and the server and explorer agree on the renamed reason value (the existing contract test in `coffret-server/src/routes/work/contract.rs` and the explorer's tests).
- [x] A finding for a discarded copy reads `discarded <path>: …` in the CLI's human and `--json` output (tests updated).
