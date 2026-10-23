---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! test -e frontend/packages/apps/web/src/awaiting.ts && ! grep -rqE "awaitingFolders|awaitingIn|AWAITING\b" frontend/packages/apps/web/src/ && grep -q "prospective folder" docs/concepts/library/README.md && grep -rq "unknown" frontend/packages/apps/web/src/dropTarget.ts'
assignee: null
branch: task/1023-1818-fix-the-explorers-small-defects-and-name-prospective-folders
created_at: 2026-10-23T09:18:43Z
updated_at: 2026-10-23T10:20:00Z
---

# fix(explorer): name the folders the Library does not hold yet, and mend seven small defects of the tree, the list and the drop

## Overview

Dogfooding the explorer since the Electron move (x7c1/coffret#332, #341) left a
set of small defects in the web app (`frontend/packages/apps/web/src/`). Each is
one decision already taken; implement them as written. All of them are in the
explorer's page; the server is not touched except where a criterion says so.

### 1. Name the folders the Library does not hold yet: **prospective folder**

The explorer draws two kinds of folder the Library does not hold — an **empty
mapped prefix** and a **prepared path** — and the concept doc
(`docs/concepts/library/README.md`, "Browsing surface") names each but has no
term for the pair. The web code calls the pair `awaiting` (`awaiting.ts`,
`awaitingFolders`, `awaitingIn`, the `awaiting` prop of `FolderTree`, the
`AWAITING` mark), which collides with the Library concept's *awaiting
relocation* (a file waiting to be moved after a rename): the same word for two
unrelated things.

- In the concept doc, introduce **prospective folder** as the term for both: a
  folder the explorer draws that the Library does not hold yet, offered as a
  place a drop can land. Keep the two kinds as its sub-terms. One or two
  sentences in Browsing surface where the pair is introduced; do not restructure
  the section.
- Rename the module to `prospective.ts` and every identifier, prop, comment and
  test name that says `awaiting` for this pair to say `prospective`
  (`prospectiveFolders`, `prospectiveIn`, `PROSPECTIVE`, the prop
  `prospective`, …). Leave *awaiting relocation* alone wherever it genuinely
  means relocation.
- Only this repository's concept doc is changed here; no translated copy of
  it is edited.

### 2. Forget one prepared path from its row

A path prepared with "New folder…" that was mistyped stays in the tree and the
list until a reload, and a reload forgets every other prepared path too. Add a
"Forget this folder" item to the "⋮" menu of a row that is a prepared path (in
the list and in the tree; `RowMenu`). It removes that path, and every path
prepared under it, from the page's state and nothing else — nothing is sent,
nothing on disk changes. It is offered only on a prepared path, never on an
empty mapped prefix (a mapping is device state, not the page's) or on a folder
the Library holds.

### 3. No "Pack this folder…" on an empty prospective folder

`FileList.tsx` offers "Pack this folder…" on the folder the list shows. On a
prospective folder nothing is there to pack, so the item is pointless. Leave
it out when the listing is of a prospective folder (`held` is false) and holds
no file and no folder on disk. Keep it where files are standing on disk under a
folder the Library does not hold yet (a drop just landed there).

### 4. Keep the tree true after the last Entry of a top-level folder is deleted

`App.tsx` reads the root's listing once (`rootListing`, `rootKnown`) and does
not read it again while the screen is on a folder other than the root. Deleting
the last Entry of a mapped top-level folder from a deeper folder's screen leaves
the tree without that folder until the root is visited, since the folder is
now an empty mapped prefix that only the root's listing names. After a deletion
has ended (where `reloadListing` and `reloadFolders` are called in the effect
around `App.tsx:873`), reload the root's listing too when the screen is not on
the root.

### 5. The tree stays neutral while the root's listing is unknown

`FolderTree.tsx:323` treats a folder whose mapping is not known yet
(`drops.mapped(path) === null`, before the root's listing has answered) as
`mapped`, so every row shows the "taken" colour the instant a drag starts, and
the rows that are not mapped flip to the refused colour when the listing
arrives. Add an `unknown` outcome to `DropOutcome` (`dropTarget.ts`): the
outline (`dropOutline` in `Banner.tsx`) is a neutral colour for it, and the
drop banner's line says the answer is not in yet rather than promising a drop.
`dropOutcome` takes `mapped: boolean | null`. A drop let go while unknown is
treated as today (the server's answer decides).

### 6. A drop onto a folder standing on disk is not "new"

The confirmation (`DropConfirm`, via `destinationsOf` and `namedBy` in
`destination.ts`) marks a level as new when the Library does not name it. A
folder the Library does not hold but that stands on the device's disk under a
mapped folder — the listing's `folders_on_disk` — is also marked new, yet a
freeze of it takes the files already standing there along with the drop. Give
a `Level` a third state for that: the confirmation says the folder is on this
device's disk and not in the Library yet, and that this drop adds it together
with what is already in it. `namedBy` keeps answering whether the Library
names a path; the on-disk answer comes from the listing of the folder let go
over (`folders_on_disk`, which names the folders directly in it).

### 7. The list shows its keyboard focus

The list (`FileList.tsx:408`, `tabIndex={0}`) draws no outline of its own, so
tabbing into it changes nothing on the screen when no row is selected. Keep the
outline for a drag alone, as the comment at `FileList.tsx:414` wants, and
instead select the first row when the list gains focus from the keyboard and
no row is selected. A focus given by a click keeps selecting the clicked row,
as today.

### 8. Deleting the open book from the tree moves the list to its parent

Deleting, from the tree's "⋮", the folder the list is showing leaves the list
on a path the Library no longer holds ("the Library holds nothing at this
path"). When the deletion that ended is of the folder on the screen, or of a
folder above it, navigate to the nearest folder above that the Library still
holds (the root where none does). The notice that the deletion ended is still
shown.

### Out of scope

- Forgetting a prepared path on any other trigger than its own "⋮".
- Any change to what the server lists.
- The explorer's session notice: the doubled "holds no session" line was fixed
  in x7c1/coffret#305 (see `Explorer.test.tsx`, "says once").

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `docs/concepts/library/README.md` names **prospective folder** in Browsing surface as the term for an empty mapped prefix and a prepared path, and `make spec-citations` passes (`grep -q "prospective folder"` is appended to `check_command`).
- [x] `frontend/packages/apps/web/src/awaiting.ts` is gone and no identifier `awaitingFolders`, `awaitingIn` or `AWAITING` remains under `frontend/packages/apps/web/src/` (both gates appended to `check_command`); the vitest suites that covered them pass under their new names.
- [x] Web vitest: "Forget this folder" is in the "⋮" of a prepared path's row in the list and in the tree, removes that path and the paths prepared under it from both, sends nothing (no request is made), and is absent on an empty mapped prefix and on a folder the Library holds.
- [x] Web vitest: "Pack this folder…" is absent on a listing that is not held and has neither files nor folders on disk, and present on one that is not held but has files standing on disk.
- [x] Web vitest: after a deletion ends while the screen is on a folder other than the root, the root's listing is requested again (and not when the screen is on the root, where the folder's own reload covers it).
- [x] Web vitest: with the root's listing not yet answered, a drag over a tree row shows the neutral outline and the "not known yet" line; once the root's listing answers, a mapped row shows the taken colour and an unmapped one the refused colour. `dropOutcome({ mapped: null, … })` is `'unknown'` (`grep -rq "unknown" dropTarget.ts` is appended to `check_command`).
- [x] Web vitest: a drop onto a folder the listing names in `folders_on_disk` is confirmed with the line that says it is on disk and not in the Library yet, and not with the "is new" line; a folder neither named nor on disk is still "new".
- [x] Web vitest: tabbing into the list with no row selected selects the first row; a click on a row still selects that row.
- [x] Web vitest: when the deletion of the folder the list is showing ends, the list moves to the nearest folder above that the Library holds (the root where none does), and the deletion's notice is shown; deleting a folder elsewhere leaves the screen where it is.

### Before merge (verified outside the check command)

- [x] On this macOS machine, with `make e2e-it` (or the Electron e2e target where it covers the explorer): a drop onto a prepared path still lands as it did before the rename, so that the rename changed no behaviour. The merging session records the run in the PR.
