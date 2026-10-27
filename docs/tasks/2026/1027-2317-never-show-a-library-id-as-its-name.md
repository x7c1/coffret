---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, spec-alignment, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1027-2317-never-show-a-library-id-as-its-name
created_at: 2026-10-27T14:17:00Z
updated_at: 2026-10-27T15:46:00Z
---

# fix: never show a Library ID as its name, and treat two directories of one Library as one fault

## Overview

#371 gave a Library one name, kept on Storage, and named its directory on a
device by its Library ID. A Library may have no name yet: one joined before
its catalog caught up, or one an earlier build made that no device has
opened with this build. Several places still treat such a Library's ID, or
something else, as its name. Close them, and the gaps in finding a Library
by name that the same change left.

### Behavior

1. **Advice that names a command.** Where coffret suggests a command for a
   Library — `coffret authorize --library …` for a grant that is gone or an
   account that is missing (`coffret-device/src/error/display.rs`, the
   advice in `coffret-cli`), and any other advice built the same way — a
   Library with no name is named with `--library-id <id>`, not with
   `--library <id hex>`. A Library with a name keeps `--library <name>`.
2. **Reports and the explorer.** Where a report, the CLI's text or JSON
   answer, or the server's API shows an opened Library's name and the
   Library has none (`OpenLibrary::name` is `None`), it says the Library has
   no name yet and gives its ID as the ID, rather than putting the ID where
   the name goes. Find every consumer of the name; JSON keeps `library: null`
   beside `library_id`.
3. **A directory being moved.** While a command holds the server lock only
   to move a Library's directory to its ID (LA-8), a second command that
   finds the lock held reports `LibraryAlreadyServed` with the mover's pid,
   as if a server held the Library. Say that the Library's directory is
   being moved, and that the command can be run again once that finishes.
   The lock file may record what holds it; keep LA-8's guarantees.
4. **A failed join.** A join that fails reports
   `the Library "<Drive folder id>" was not joined` (`LibraryNotJoined` in
   `error/display.rs`), naming the folder as if it were the Library. A join
   names no Library; say which folder or Storage location it was joining,
   by what it is.

### Finding a Library

5. **Two directories of one Library.** `library_dir::find` and
   `LibraryDir::libraries` list every directory, so two directories whose
   settings carry the same Library ID — a copy, or the earlier build's
   directory beside the moved one — both answer `--library <name>` and both
   appear in the Passphrase window's list. A device holds a Library once:
   treat two directories with one ID as a fault, refused with both paths
   and the ID by `--library` and `--library-id` alike, and list the Library
   once in `libraries()`, marked as needing attention, rather than twice.
6. **`scripts/drive-it-reset.sh`.** `library_label` labels an unnamed
   Library by its ID and lists one Library twice where two directories hold
   it. Label it by the name copy in its settings, or say it has none, and
   list each Library ID once.

### Structure

7. **`LibraryName`'s derives** (`coffret-model/src/library_name.rs`): keep
   only those something uses.

Update the spec rules (FM-15, LA-8 and any that describe the finding of a
Library by name) and the CLI help where the behavior above changes them.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `coffret-device` error tests: for a Library with no name, the
      gone-grant and missing-account advice suggests
      `coffret authorize --library-id <id>`; for a named one,
      `--library <name>` as before
- [x] A test shows an unnamed opened Library reported as having no name,
      with its ID as the ID, in the CLI text and JSON answers and in the
      server's API
- [x] `coffret-device` test: a command that meets the lock held by a
      directory move gets a refusal that says the directory is being moved,
      not `LibraryAlreadyServed`
- [x] `coffret-device` error test: a failed join's message names the folder
      or Storage location, and no Library name
- [x] `coffret-device` test: two directories whose settings carry the same
      Library ID are refused by `--library` and `--library-id` with both
      paths, and `libraries()` lists the Library once
- [x] `make check` passes

### Before merge (verified outside the check command)

- [ ] On a throwaway state directory under `.tmp/` against the local MinIO,
      with `COFFRET_STATE_DIR` and `COFFRET_LOG_DIR` set: a copy of a
      Library's directory beside it is refused with both paths by
      `coffret sync --library <name>`, and a Library joined before its
      catalog caught up is named in `coffret authorize`'s advice by
      `--library-id`
