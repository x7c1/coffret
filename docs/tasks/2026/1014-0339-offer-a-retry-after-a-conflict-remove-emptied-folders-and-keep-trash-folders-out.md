---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "emptied" docs/spec/entry-path/README.md'
assignee: null
branch: task/1014-0339-offer-a-retry-after-a-conflict-remove-emptied-folders-and-keep-trash-folders-out
created_at: 2026-10-13T18:39:00Z
updated_at: 2026-10-14T08:20:00Z
---

# fix(delete,sync): offer a retry after a conflict, remove the folders a deletion emptied, and keep desktop trash folders out of the listing and the drop

## Overview

Four findings around deleting from the explorer (PRs #265, #267, #268) and
what the device does afterwards.

### 1. A deletion refused as `conflict` offers to run again

When the preview or the arming of a deletion is refused, the explorer puts
one sentence in the list's notice area (`App.tsx:719`,
`${targetName(target)} was not deleted — ${said(cause)}`), and when a run
stops the same shape comes from `deletedLine` (`deleteEntries.ts:216-217`).
A `conflict` refusal — the server's "another device changed the Library
meanwhile, so nothing was changed — running it again plans from the Library
as it is now" (`api_error/server.rs:114-123`) — says to run it again but
gives nothing to press; the person has to find the row's Delete again. For a
refusal whose kind is `conflict`, the notice carries a `try again` button that
calls `deleteEntries` with the same target; other refusals stay as they are.
Keep the notice's shape in `deleteEntries.ts` (a notice is `{ text, retry? }`,
or similar) with its tests, and render the button in `App.tsx`.

### 2. The refused-Pack sentence counts its files

`RefusedPack::message` (`coffret-server/src/delete/delete_run.rs:121-132`)
always says "the Pack holding them" and "the files packed beside them", also
when the target is one file. Make the sentence agree with the number of
targeted files in that Pack (it, them; the file, the files), the way
`deleteEntries.ts:112-116` already varies `stays` / `stay` on the web side.

### 3. A folder the deletion emptied on the device is removed

After a folder is deleted from the Library, the next sync moves the
device's unedited copies to the desktop's trash (EP-15,
`coffret-usecase/src/sync/run.rs:287-318` `move_departed_to_trash`,
`coffret-local-fs/src/unix_destinations/trash.rs`), and the now-empty
`album-000/` stays under the mapped root. Nothing removes directories
(`remove_dir` appears nowhere under the usecase or the local fs gateway).

Remove what this run emptied, and only that: a directory below the mapped
root that, after this run's moves, holds nothing at all is removed, and the
check repeats upward until a directory that still holds something or the
mapped root itself. A directory that was already empty before the run, or
holds a file the run did not move (an edited copy, a file of the person's,
a dotfile), stays. The removal goes through the local fs gateway next to the
trash move, under the same `openat`-style walk `vouched()` uses, and never
follows a symlink. Say it in EP-15 in one sentence under what the rule does
(the gate in `check_command` greps `emptied` in the Entry Path spec), and
add the case to the sync conformance suite
(`coffret-usecase/src/sync_conformance/departed.rs`), including the two
negative cases above.

### 4. Desktop trash folders are kept out of the listing and the drop

EP-16 excludes `.Trash`, `.Trashes` and `.Trash-<digits>` as the first
component under a mapped root from the scan and the fetch
(`coffret-usecase/src/trash_folders.rs` `is_trash_folder`, used in
`local_scan/walk_mappings.rs:209` and `fetch/local_place.rs:93-99`). The
listing's `folders_on_disk` (`coffret-device/src/add/added_locally.rs:163`,
which skips only the scratch and management names) and the upload's
`reserved()` check (`coffret-device/src/add/add_file.rs:141-143`) do not know
the names, so a mounted volume's trash shows in the explorer's list of folders
on disk, and a drop into it is accepted. Use `is_trash_folder` in both: the
listing leaves such a folder out, and the upload refuses a drop whose
destination's first component under the root is one, with the refusal the
scratch prefix gets today. Mind the prefix wrinkle `local_place.rs:86-92`
explains: the first component *under the mapped root*, not of the Entry Path.
Add the cases to `coffret-device/src/add/tests.rs`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `deleteEntries.test.ts`: a `conflict` refusal yields a notice with a
      retry, any other refusal a notice without one; the stopped-run line
      for a conflict carries the retry too
- [x] A server unit test on `RefusedPack::message`: one targeted file reads
      "the Pack holding it" and "the file packed beside it"; two read the
      plural
- [x] Sync conformance: after a folder's files are moved to the trash, the
      emptied folder and its emptied parents below the mapped root are gone;
      a folder that also holds an edited copy stays; a folder that was empty
      before the run stays; the mapped root itself is never removed
- [x] `coffret-device` add tests: `.Trash` at the top of a mapped root does
      not appear in `folders_on_disk`, and an upload to `.Trash/x.jpg` under
      that root is refused with the same kind as the scratch prefix; with a
      mapping prefix the check still looks at the first component under the
      root
- [x] `make check` passes; EP-15 mentions the emptied folders (gate in
      `check_command`)

### Before merge (verified outside the check command)

- [ ] On the agent's Library (`.tmp/agent/up.sh 1`, headless Chromium):
      delete `lib/album-000` from the explorer, run `coffret sync` on the
      same device, and `.tmp/agent/root/album-000/` (the mapping `lib` → `.tmp/agent/root`) is gone while
      `.tmp/agent/root/` remains; create `.tmp/agent/root/.Trash/` and
      it is not listed under the root in the explorer
