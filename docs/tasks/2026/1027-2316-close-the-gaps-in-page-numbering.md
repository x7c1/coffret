---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1027-2316-close-the-gaps-in-page-numbering
created_at: 2026-10-27T14:16:43Z
updated_at: 2026-10-27T14:37:28Z
---

# fix(explorer): close six gaps in page numbering and its preview

## Overview

"Number the pages" in the folder-drop confirmation
(`frontend/packages/apps/web/src/renumber.ts`, previewed by
`DropConfirm.tsx`) has six gaps found while numbering real books. Close them
in one change; each is small, and they share the rule and its preview.

### 1. Names that differ only in case, after the same page

Before the first page, `cover.jpg` and `Credits.jpg` read naturally as
`cover` → `Credits`, but `000_Credits.jpg` sorts before `000_cover.jpg` by
bytes, so `judgePages` leaves the whole folder as it was (`'order'`). Where
names put after the same page would not sort by bytes in their natural
order, give them a natural-order serial between the page number and the
name: `000_1_cover.jpg`, `000_2_Credits.jpg`. Names after the same page that
already sort correctly keep today's form (`020_note.jpg`), so existing
results do not change.

### 2. Say which names left a folder whole

When a folder's pages are left whole because the new names would not keep
the reading order, the preview gives only the reason
(`leftWholeLine`). Name the two names whose order would invert, so the
person knows which one to rename and drop again. Carry them on the
`LeftWhole` value rather than recomputing them in the component.

### 3. Number images only

Every file in a folder is treated as a page today, so `ComicInfo.xml` and
`readme.txt` become `000_ComicInfo.xml`. External readers look these up by
name. Only files with an image extension are pages — at least `jpg`,
`jpeg`, `png`, `webp`, `avif`, `gif`, `bmp`, `tif`, `tiff`, compared without
regard to case. Every other file keeps its name, takes no part in the
sequence, the width, the order check or the "same value twice" check, and
is listed among the unchanged names. A folder with no image is not a folder
of pages.

### 4. List unchanged pages in reading order

A page already named as its number (`001.jpg`) is listed as "unchanged" at
the end of its group, so a page from the middle of the book shows up last.
In `DropConfirm.tsx`'s `Preview`, list renamed and unchanged pages together
in the reading order (the natural order of the dropped names). Non-page
files from item 3 come after the pages.

### 5. Say why Add is not available yet

After correcting a volume folder's name, the dialog reads the Library under
the corrected path before Add can be pressed; a click in that window is
dropped, and the "reading" line sits at the top of the dialog, out of view
when the preview is long. Show the reading state, and a replacement warning
if the read turns one up, beside the Add button as well, so a press that
does nothing visibly says why. The click itself stays dropped.

### 6. A bare second number is a spread only when it is the next page

Since #370 a spread's second number may drop the frame's leading text
(`foobar19_0018+0019.jpg`). That also reads `p0018-2.jpg`, a scanner's
second take, as a spread and renames it `018-002.jpg`, which then repeats
18 beside `p0018.jpg` and leaves the folder whole. A second number written
without the leading text is a spread only when its value is the first
number plus one: `p0018+0019.jpg` and `foobar19_0018-0019.jpg` still are,
`p0018-2.jpg` is not and is put after page 18 as any other name
(`018_p0018-2.jpg`). A second number written with the full frame keeps
today's rule.

Update the `renumber.ts` header comment wherever it describes these rules.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `renumber.test.ts`: `cover.jpg, Credits.jpg, p001.jpg, p002.jpg`
      become `000_1_cover.jpg, 000_2_Credits.jpg, 001.jpg, 002.jpg`, and a
      folder whose names after the same page already sort by bytes is
      numbered as before (existing tests pass unchanged)
- [x] `renumber.test.ts`: a folder left whole for its order names the two
      names that would invert, and `DropConfirm.test.tsx` shows them in the
      left-whole line
- [x] `renumber.test.ts`: `ComicInfo.xml` and `readme.txt` beside numbered
      pages keep their names and do not change the pages' width or order;
      `P001.JPG` is a page
- [x] `DropConfirm.test.tsx`: `001.jpg` (unchanged) is listed between the
      renamed pages before and after it, and non-page files after the pages
- [x] `DropConfirm.test.tsx`: while the Library is being read after a
      correction, the reading state is shown next to Add
- [x] `renumber.test.ts`: `p0017.jpg, p0018.jpg, p0018-2.jpg, p0019.jpg`
      become `017.jpg, 018.jpg, 018_p0018-2.jpg, 019.jpg`; `p0018+0019.jpg`
      and `foobar19_0018-0019.jpg` are still spreads
- [x] `make check` passes

### Before merge (verified outside the check command)

- [x] On the agent's Library, headless Chromium with a one-off Playwright
      script that drops a folder the way `07-freeze.spec.ts`
      `dropFolderOnto` does (`.tmp/agent/v368-drop.mjs` is a working
      example): dropping `cover.jpg, Credits.jpg, ComicInfo.xml, p001.jpg …
      p010.jpg, p010-2.jpg` with "Number the pages" ticked lands
      `000_1_cover.jpg, 000_2_Credits.jpg, ComicInfo.xml, 001.jpg … 010.jpg,
      010_p010-2.jpg`
