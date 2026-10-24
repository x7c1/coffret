---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, rust-module-structure, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "EP-17" docs/spec/entry-path/ && grep -rq "EP-17" backend/crates/domain/coffret-usecase/src/ && grep -rqF "__MACOSX" backend/crates/domain/coffret-usecase/src/ && grep -rqF "Thumbs.db" frontend/packages/apps/web/src/'
assignee: null
branch: task/1024-1420-leave-the-files-an-operating-system-keeps-out-of-the-library
created_at: 2026-10-24T05:20:00Z
updated_at: 2026-10-24T06:22:37Z
---

# feat: leave the files an operating system keeps out of the Library

## Overview

Files an operating system writes beside a person's own — `.DS_Store` from the
Finder, `Thumbs.db` from Windows Explorer, AppleDouble `._*` files on volumes
without extended attributes — are taken into the Library today like any other
file: a drop carries them in, and a sync or a freeze of a mapped folder makes
them Entries. Once one is inside a Pack, removing it means rewriting the whole
Pack (PK-10), so the right place to keep them out is before they become
Entries. This task adds that rule, applies it on every path a file enters by,
and says so quietly on the screen.

### Decision (already made with the person; implement it)

**The names.** A fixed list, compared byte for byte — case included — as
EP-16 compares the trash folders (`backend/crates/domain/coffret-usecase/src/trash_folders.rs`):

- Files left out at any depth: `.DS_Store`, any name starting with `._`
  (AppleDouble), `Icon\r` (the four letters followed by a carriage return —
  the Finder's custom-folder-icon file), `Thumbs.db`, `ehthumbs.db`,
  `desktop.ini`, `.directory` (KDE).
- Folders left out at any depth: `__MACOSX` (what unpacking a Finder-made zip
  leaves).
- Folders left out only as the first component directly under a mapped root
  or directly in a dropped folder: `.Spotlight-V100`, `.fseventsd`,
  `.TemporaryItems`, `$RECYCLE.BIN`, `System Volume Information`.
- A name of the person's own that happens to start with `._` is left out too;
  that cost is accepted and stated in the spec, as EP-14 and EP-16 state
  theirs.

**Where it applies.** Every path a file becomes an Entry by:

- The scan of mapped folders (`backend/crates/domain/coffret-usecase/src/local_scan/walk_mappings.rs`)
  — so sync, freeze and the "Pack this folder…" preview. Decided from the name
  before what it is, as the trash-folder check there is; a left-out folder is
  never entered.
- The desktop app's drop walk (`backend/crates/domain/coffret-usecase/src/drop_walk/`),
  LA-29.
- A browser's drop (`frontend/packages/apps/web/src/drop.ts`, `droppedFiles`),
  filtered on the page before anything is sent; and the upload route
  (`backend/crates/apps/coffret-server/src/routes/upload/`) declines such a
  path if one arrives anyway, as it declines `trash_folder`.

Keep the rule in one I/O-free module in `coffret-usecase`, beside
`trash_folders.rs` and in its style. The page needs the same list for the
browser drop; keep one source for it (for example a JSON list both sides
read, or a TypeScript copy pinned by a test against the Rust list), and say in
the module docs which side is the source.

**What the screen says.**

- The drop confirmation (`frontend/packages/apps/web/src/DropConfirm.tsx`)
  counts and sizes the drop without them, and adds one line in the dim colour
  (`COLOR.dim`) such as `left out 3 files the system keeps (.DS_Store, Thumbs.db)`
  — distinct names, a few at most. It is **not** part of the warning-coloured
  "not taken" line (`notTakenNotice` in `drop.ts`): these files are in almost
  every drop, and a warning would read as something gone wrong. The same
  applies where a drop is answered without the confirmation (`withNotTaken`).
  The walk therefore reports them apart from the other not-taken reasons (a
  reason of its own travelling to the page, or a separate list — your choice;
  LA-26 says what the page is told of a drop, so amend it to match).
- "Pack this folder…" (`frontend/packages/apps/web/src/packFolder.ts`,
  `leftOutParts`) adds the same group to "left as they are", which needs the
  freeze preview to count what the scan left out by this rule.
- Sync and freeze results say nothing about them, as for EP-14 and EP-16.

**Spec and concept.**

- Add **EP-17** to `docs/spec/entry-path/README.md`, after EP-16 and in its
  shape: the names, the depth rule, scan/fetch/drop behaviour, the cost, and
  *(Form: test)*. A fetch of an existing Entry at such a path is not changed.
- An Entry already in the Library at such a path (carried in before the rule)
  stays until it is deleted, and a scan that no longer finds its file does not
  report it as deleted locally (EP-10). The person's production Library holds
  none, so this is one sentence in EP-17 plus the code that makes it so.
- EP-12 (whether a root holds nothing) and EP-15 (a folder emptied by
  discards) are **not** changed: a folder holding only such files still holds
  something.
- Amend LA-26 / LA-29 in `docs/spec/loopback-access/README.md` for the drop
  walk, and the Library concept (`docs/concepts/library/README.md`) where it
  says what a scan and a drop take.

### Out of scope

- Removing such files from the device's folders, or removing Entries already
  in a Library.
- A configurable or user-editable list.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] One module in `coffret-usecase` answers, from the name alone, whether a
      name is left out, with unit tests for every listed name, for the
      depth-limited folders at the top and deeper, and for look-alikes that
      are kept (`.ds_store`, `Thumbs.DB`, `thumbs.db`, `_MACOSX`, `Icon`,
      `.Spotlight-V100x`) — gates `__MACOSX` and `EP-17` appended.
- [x] A scan of a mapped folder holding these files and folders (at the top
      and deeper) yields no Entry for any of them and nothing reported about
      them; a sync then a freeze of such a folder packs only the person's files.
- [x] An Entry already at such a path is not reported as deleted locally by a
      scan that does not find its file.
- [x] The drop walk leaves them out and reports them apart from the other
      not-taken reasons; the upload route declines such a path.
- [x] The page filters a browser drop by the same list (gate on `Thumbs.db`
      in `frontend/packages/apps/web/src/` appended), and vitest covers the
      confirmation's dim line, its absence when nothing is left out, and the
      warning line staying for the other reasons.
- [x] The freeze preview counts what the rule left out, and "Pack this
      folder…" names that group in "left as they are".
- [x] EP-17 is in the entry-path spec (gate appended), and LA-26 / LA-29 and
      the Library concept say the same.

### Before merge (verified outside the check command)

- [x] On this machine, in a throwaway Library on the local MinIO under a state
      directory of the agent's own (never `books` or the production Library):
      map a throwaway folder holding a
      book and `.DS_Store`, `._page-001.jpg`, `Thumbs.db` and a `__MACOSX/`
      folder, sync, and confirm with `make cli ARGS="ls …"` that only the
      book's files became Entries. The merging session records the commands
      and output in the PR.
- [ ] Needs a person: the dim line on the drop confirmation reads well beside
      the rest of the dialog (taste).
