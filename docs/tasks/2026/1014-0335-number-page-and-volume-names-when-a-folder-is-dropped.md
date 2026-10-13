---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && test -f frontend/packages/apps/web/src/renumber.test.ts'
assignee: null
branch: task/1014-0335-number-page-and-volume-names-when-a-folder-is-dropped
created_at: 2026-10-13T18:35:00Z
updated_at: 2026-10-13T21:45:00Z
---

# feat(explorer): offer to number page and volume names when a folder is dropped

## Overview

Scanned books arrive with page names that differ from one job to the next:
`king01-#001.jpg … king01-#210.jpg`, `第1巻/… 第12巻/…`, `p-0001.jpg`, with
the title in the name or not. Once they are in the Library, nothing lines
them up: a listing sorts by bytes (EP-3), and a rename after the fact would
need the Journal's `renames`, which the explorer does not have yet. Names
are decided before anything is sent, though — the explorer puts the relative
path of each dropped file in the multipart file name (`gateway/api/src/upload.ts`
`addFiles`, `:254`) and the server takes it as given (`routes/upload/receive.rs`
`:74`) — so numbering at drop time needs no server change.

Add two checkboxes to the folder-drop confirmation (`PackConfirm.tsx`, below
the Pack / one-by-one choice): **Number the pages** and **Number the volume
folders**. Both are off by default and remembered across drops. When one is
checked, the dialog previews the result: the first and last few renames as
`king01-#001.jpg → 001.jpg`, and the names it leaves alone, with a way to
expand to the whole list. Whichever choice is then made — Pack or one by one —
the files go up under the new names. Cancel sends nothing, as today.

### The rule

A pure module `renumber.ts` in `apps/web/src` (DOM-free, with a sibling
`renumber.test.ts`, in the style of `packChoice.ts`) takes the dropped
`Added[]` and the two options and returns the files with their new paths plus
what the preview shows. The rule, decided with the user:

- **Pages** are files judged among their siblings in the same folder;
  **volumes** are folders judged among their sibling folders under the same
  parent. The dropped folder's own name (the first path component) is never
  changed: it is the book or series, not a volume.
- **The number is the last run of digits** in the name (for a file, the name
  without its extension). The candidates in a group are those names whose
  text before and after that run is the same across the group — the run's
  *frame*, which may be empty on either side (`king01-#` / ``, `第` / `巻`).
  When the siblings fall into several frames, only the most populous frame
  is renamed; on a tie nothing in that group is renamed.
- **The new name is the number, zero-padded.** The value is kept, never
  reassigned: gaps stay gaps, and a volume added later (`vol11` → `011`) or a
  page found later lines up with what was numbered before. The width is three
  digits, widened only when the largest value in the group needs more
  (`1000` → four digits for the whole group).
- **A page keeps its extension, lowercased** (`.JPG` → `.jpg`). A volume has
  none.
- **Nothing is renamed in a group** where two candidates carry the same value
  (`01` and `1`), or where a new name would equal the name of a sibling that
  is not being renamed (a file called `001.jpg` beside `king01-#001.jpg`).
- **Names without a number are left as they are** (`cover.png`) and shown in
  the preview as unchanged. Nothing is appended to the end of the sequence.

Dropped loose files (no folder, `summarize` returns null in `packChoice.ts`)
are sent without asking, as today, and are not numbered.

### Wiring

- `chooseAndAdd` (`packChoice.ts:168`) asks and then sends `files` unchanged.
  Let the dialog's answer carry the two options beside the `Choice`, apply
  `renumber` to `files` before `send`, and keep `Choosing.ask` returning
  `cancel` on cancel. `summarize` keeps counting the original files; the
  preview is computed from the same `Added[]` the request will use.
- **Remembering the choice.** The explorer has no persisted preference yet
  (no `localStorage` use anywhere). Add a small `remembered.ts` helper that
  reads and writes one boolean under a named key, wraps every access in
  `try`/`catch` (a private window or blocked storage must not break the
  drop), and falls back to `false`. Use it for the two checkboxes.
- The preview lists at most the first three and last three renames of each
  group with the count in between, and the unchanged names; "show all"
  expands. Names are shown as `<from> → <to>` relative to the dropped folder.
- The e2e journey `07-freeze.spec.ts` drops a flat book; it keeps passing
  with both boxes unchecked, which is the default. No e2e change is required;
  if you add one, keep it under `apps/e2e/journeys` and do not add it to the
  check command.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `renumber.test.ts`: `king01-#001.jpg … king01-#210.jpg` become
      `001.jpg … 210.jpg`; `第1巻 … 第12巻` folders become `001 … 012`; a gap
      in the sequence stays a gap; `.JPG` becomes `.jpg`; a group whose
      largest value is `1000` is padded to four digits
- [x] `renumber.test.ts`: `cover.png` beside numbered pages is unchanged and
      reported as such; a group with `01` and `1` is left whole; a group
      whose new name would collide with an unrenamed sibling is left whole;
      when two frames tie in size nothing is renamed, and when one frame has
      more members only it is renamed
- [x] `renumber.test.ts`: the dropped folder's own name is never changed;
      volumes are judged among sibling folders only, pages among sibling
      files only; with both options off every path is returned unchanged
- [x] `PackConfirm.test.tsx`: the two checkboxes render unchecked by default,
      checking one shows the preview with `<from> → <to>` lines and the
      unchanged names, and the chosen options reach `onChoose` with the
      choice
- [x] `packChoice.test.ts`: `chooseAndAdd` sends the renumbered paths when an
      option is on and the original paths when both are off, for Pack and
      for one by one
- [x] `remembered.test.ts`: a stored value is read back, and a storage that
      throws yields the default without throwing
- [x] `make check` passes and `renumber.test.ts` exists (both in
      `check_command`)

### Before merge (verified outside the check command)

- [x] On the agent's Library (`.tmp/agent/up.sh 1`, headless Chromium with a
      one-off Playwright script that builds a fake folder drop the way
      `07-freeze.spec.ts` `dropFolderOnto` does): dropping a folder holding
      `vol1/king-#1.jpg … king-#12.jpg`, `vol2/…` and a `cover.png` with
      both boxes checked lands `001/001.jpg …`, `002/…` and `001/cover.png`
      in the Library; the listing sorts the pages in order; the boxes are
      checked again on the next drop
- [ ] Whether the preview reads clearly with a long book — needs a person:
      how much of the list to show is a judgement of taste
