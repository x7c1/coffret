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
branch: task/1106-0946-let-the-tree-take-keys-the-page-holds-no-focus-for
created_at: 2026-11-06T00:46:57Z
updated_at: 2026-11-06T01:12:22Z
---

# fix(web): let the tree take the keys when nothing on the page holds the focus

## Overview

The explorer's tree answers its keys (`↑` `↓` `←` `→`, `Enter`, `Escape`,
Shift with `↑`/`↓`) only while the tree element itself has the focus:
`onKeyDown` in `frontend/packages/apps/web/src/TreeTable.tsx` returns early
unless `event.target === event.currentTarget`. A click on something that
cannot take the focus, such as the reader's picture, the empty space below the
rows, the details pane's text, or the Activity panel's background, moves the
focus to `document.body`. After that, `↑` and `↓` do nothing until the person
clicks a row again. In the reader this breaks reading on with the arrows after
a click on the page being read.

Make the tree take the keys that arrive while the focus is on `document.body`
(nothing on the page holds it), wherever the click that put it there was. Do
not do it by keeping clicks from taking the focus away. Listen for the keys
that reach `body`.

- Keys whose target is `document.body` are handled as if the tree had the
  focus: the same arrows, `Enter`, `Escape` and Shift-arrow behaviour, with the
  reader's read-on (`onReadOn`) when the reader is open. Handling a key this
  way also gives the tree the focus back, so the next key goes to it directly
  and the focus ring shows where the keys go.
- Keys are **not** taken when the focus is in an input, a textarea, a select,
  a button, a link, a menu, a dialog or any other element that can hold the
  focus. Those keys stay with that control, as today.
- A key that something else has already handled (`event.defaultPrevented`) is
  not handled again. In particular, `Escape` while the reader is open still
  closes the reader sheet (the window listener in `ReaderView.tsx`), and it
  must not also climb or let go of the tree's focus in the same press.
- Nothing is assigned to a click on the reader's picture. It does not turn the
  page.
- While a modal dialog (Move to…, the drop confirmation, and so on) is open,
  `body` keys are not taken by the tree.

Out of scope: changing what any key does in the tree itself, or the `Escape`
behaviour that climbs folders (a separate change).

States the key can arrive in, each covered by a test:

- focus on `body` after a click on empty space, reader closed: `↓` moves the
  tree's focus to the next row, and the tree element then holds the focus;
- focus on `body` with the reader open: `↓` reads on to the next file, as the
  same key on the focused tree does;
- focus in a text input (the drop's numbering field, a rename field): the
  arrows are left to the input;
- focus on a button outside the tree: `Enter` is left to the button;
- `Escape` with the reader open and the focus on `body`: the reader closes and
  the tree's focus does not move;
- a dialog open: `body` keys do nothing to the tree.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Web unit tests (`frontend/packages/apps/web/src/`) cover each state
      listed in the Overview: a key whose target is `document.body` drives the
      tree (and the reader's read-on when it is open) and returns the focus to
      the tree element; keys in inputs, on buttons, inside an open dialog, or
      already handled are left alone.
- [x] `Escape` with the reader open and the focus on `body` closes the reader
      without moving the tree's focus, under a unit test.
- [x] `make check` passes.
