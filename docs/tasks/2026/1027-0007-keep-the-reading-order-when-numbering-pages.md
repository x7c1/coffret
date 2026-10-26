---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1027-0007-keep-the-reading-order-when-numbering-pages
created_at: 2026-10-26T15:07:00Z
updated_at: 2026-10-26T21:42:33Z
---

# feat(explorer): keep a book's reading order when its pages are numbered at drop time

## Overview

A scanned book is read in the order its file names sort. "Number the pages"
in the folder-drop confirmation (`frontend/packages/apps/web/src/renumber.ts`)
renames only the names in the most common frame and leaves every other name
as it is. That breaks the order whenever a name that is not a plain page
number sits at the start or in the middle of the book:

```
cover.jpg, p0001.jpg … p0017.jpg, p0018+p0019.jpg, p0020.jpg …
```

Today `p0001` … `p0020` become `001.jpg` … `020.jpg`, while `cover.jpg` and
the joined spread `p0018+p0019.jpg` keep their names. The Library sorts by
bytes (`inLibraryOrder`), and digits sort before letters, so both move to the
end of the book.

Change the page rule so that, after numbering, the byte order of the new names
is the order the person saw in their file manager. The number values are still
kept, never reassigned. The volume-folder rule does not change.

### The rule for pages

The rule is judged per folder, among sibling files, as today.

1. **The order to keep is the natural order of the original names**, which is
   what a file manager shows. Runs of digits compare by value, and case is
   ignored. Use an `Intl.Collator` with `numeric: true` and
   `sensitivity: 'base'` under a fixed locale, so that the result does not
   depend on the machine. Names the collator calls equal fall back to byte
   order.
2. **The most common frame keeps today's rule.** That covers the frame
   judgement, the tie (nothing renamed), the same value twice (nothing
   renamed), the width (three digits, wider when the largest value needs it,
   now counting the spreads' numbers too) and the lowercased extension.
3. **A spread** is a name whose stem is two numbers of the most common frame
   joined by `+` or `-`, each number with the frame's text around it
   (`p0018+p0019`, or `0018-0019` where the frame is empty). Both numbers are
   padded to the group's width and the joining character is kept:
   `p0018+p0019.jpg` → `018+019.jpg`.
4. **Every other name** gets a prefix: the last number of the page before it
   in natural order, then `_`, then the original name with its extension
   lowercased. This covers names without a number, and names in a less common
   frame. The page before can be a plain page or a spread.
   - `p0020.jpg`, `note.jpg` → `020.jpg`, `020_note.jpg`
   - after `p0018+p0019.jpg`, `x.jpg` → `019_x.jpg`
   - a name before the first page uses the first page's first number minus
     one: `cover.jpg` → `000_cover.jpg`
5. **If the order cannot be kept, the folder's pages are left whole**, with a
   new `LeftWhole` reason (for example `'order'`). Check this after the names
   are decided. It applies when:
   - the byte order of the new names differs from the natural order of the
     original names (for example two prefixed names after the same page, whose
     byte order and natural order disagree, `a2` and `a10`)
   - two new names are equal, or a new name equals a sibling folder's final
     name
   - the first number is `0`, so there is no value to put before it

   `leftWholeLine` says this as something like "left as they are: numbering
   them would change the order they are read in".

Shapes not yet seen in real books are out of scope and stay prefixed names
under step 4. These include three or more joined pages
(`p0018+p0019+p0020`) and a second number without the frame (`p0018+0019`).
The rule grows when a real book needs it, so keep the spread match in one
place that is easy to extend.

### Preview and wiring

- With the new rule, the prefixed names and the spreads appear under
  `renamed`. `unchanged` holds only names that keep their name (a member
  already named `001.jpg`) and the names of a group left whole. Keep the
  `Group` shape unless a change makes the preview clearer.
- Update the `renumber.ts` header comment. It says the rule is "deliberately
  narrow" and that only the most populous frame is renamed. It should describe
  the order-keeping rule and say that volumes keep the narrow one.
- `chooseAndAdd`, `heldUnder` and `DropConfirm` call `renumber` and need no
  change beyond what the new reason and preview lines require.
- Existing tests that pin the old page behaviour change with it. These are
  "leaves a name without a number alone and says so", "renames only the most
  common frame", and the `cover.png` expectations in "judges volumes among
  sibling folders and pages among sibling files" and "says where a kept name
  lands when its volume is renamed". Rewrite them to the new rule rather than
  deleting them.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `renumber.test.ts`: `cover.jpg, p0001.jpg … p0017.jpg, p0018+p0019.jpg,
      p0020.jpg` become `000_cover.jpg, 001.jpg … 017.jpg, 018+019.jpg,
      020.jpg`, and the new names in byte order are in the original natural
      order
- [x] `renumber.test.ts`: a spread joined by `-` is numbered the same way; a
      spread with an empty frame (`0018+0019.jpg`) becomes `018+019.jpg`; a
      name after a spread takes the spread's last number (`019_x.jpg`)
- [x] `renumber.test.ts`: natural order, not byte order, decides where a
      prefixed name goes (`p2.jpg`, `p10.jpg` and a `p2a.jpg` between them)
      and case does not
- [x] `renumber.test.ts`: a name in a less common frame is prefixed rather
      than left alone; a tie between frames and a value carried twice still
      leave the folder whole
- [x] `renumber.test.ts`: a folder whose first number is `0` with a name
      before it, and a folder whose new names would sort out of order, are
      left whole with the new reason
- [x] `renumber.test.ts`: volume folders are numbered exactly as before (the
      existing volume tests pass unchanged)
- [x] `DropConfirm.test.tsx`: the preview shows a prefixed name and a spread
      as `<from> → <to>` lines, and the new reason's line for a folder left
      whole
- [x] `make check` passes

### Before merge (verified outside the check command)

- [x] On the agent's Library (`.tmp/agent/up.sh 1`, headless Chromium with a
      one-off Playwright script that builds a folder drop the way
      `07-freeze.spec.ts` `dropFolderOnto` does): dropping a book holding
      `cover.jpg`, `p0001.jpg … p0020.jpg` with `p0018.jpg` and `p0019.jpg`
      replaced by `p0018+p0019.jpg`, with "Number the pages" checked, lands
      `000_cover.jpg`, `001.jpg … 017.jpg`, `018+019.jpg`, `020.jpg`, and the
      listing shows them in that order
- [ ] Whether the order matches what the person's file manager shows for
      their own books (needs a person)
