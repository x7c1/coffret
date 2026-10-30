---
status: completed
pipeline_phase: null
plan: null
base_ref: feat/explorer-v0.2
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && make e2e-it && make e2e-electron'
assignee: null
branch: task/1030-1349-draw-the-explorer-in-the-designed-light-palette-and-sizes
created_at: 2026-10-30T04:49:49Z
updated_at: 2026-10-30T05:33:28Z
---

# feat(web): draw the explorer in the designed light palette and sizes

## Overview

Part of the explorer redesign on the `feat/explorer-v0.2` integration branch.
The layout and behaviour of the redesign are in place, but the explorer is
still drawn in the old dark palette and in sizes smaller than the design, and
file rows have no icon. Bring the look in line with the design. The values
below are the design's; use them as given.

**Palette (light).** Keep `COLOR` in `frontend/packages/apps/web/src/theme.ts`
as the one named set of colours, and change its values to the light ones (plus
the ground and hover states `index.html` paints). A dark set and a switch
between the two come in a later change, so keep every colour going through a
name rather than a literal, and do not add the switch now.

| Role | Value |
|---|---|
| ground (page, tree, details pane) | `#f7f8fa` |
| panel (header, Activity panel, menus, dialogs) | `#ffffff` |
| border | `#e3e6ea` |
| rule between rows, separators inside menus | `#eef0f3` |
| text | `#16181d` |
| dim (sizes, times, the location, headings) | `#5b616e` |
| faint (placeholders, arrows in previews) | `#8a919c` |
| accent (focused row, primary buttons, file icons, progress) | `#2563eb` |
| accent, darker (text on an accent tint) | `#1d4ed8` |
| focused row | background accent, text and icons `#ffffff` |
| a row selected beside the focused one | `#dbe6fd` |
| drop target row | `#e8effd` with an inset 2px accent ring |
| menu item hover | `#eef1f5`; a destructive item's hover `#fdecea` |
| refused / failed | `#b42318` on `#fdecea` |
| warning text on its ground | `#8a6100` (text `#5c4400` in a notice box) on `#fff8e6` |
| success dot | `#15803d` |
| status icon "on this device" | `#3d424d` |

The reader sheet stays dark, as in the design: `#111214` ground, `#e6e8eb`
text, `#a3a8b0` for its secondary text, and a shadow
`-16px 0 40px rgba(0, 0, 0, 0.28)` on its left edge.

**Icons on every row.** 16 px, stroke icons (24 px viewBox, stroke-width 2,
`currentColor`), with 6 px between the icon and the name:
- folder: `M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z`, in dim;
- image file (what the reader can show as a page): a rectangle
  `x=4 y=3 w=16 h=18 rx=2`, a circle `cx=9.5 cy=9 r=1.5` and the path
  `m20 16-4.5-4.5L7 20`, in accent;
- any other file: `M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z`
  and `M14 3v6h6`, in accent.
On the focused row the icons are white.

**Sizes.**
- Header: padding `10px 16px`, gap 12 px. The Library's name in a box at least
  44 px high, padding `0 12px`, 1 px border, radius 8 px, 14 px semibold (600).
  The location 13 px in dim. The **+** a 44 × 44 px button, radius 8 px, filled
  with accent, a white 20 px plus drawn as an SVG (`M12 5v14M5 12h14`,
  stroke-width 2, round caps).
- Tree: rows at least 32 px high, names 13 px; the column headings 12 px, weight
  500, in dim, with `8px` vertical padding and a border under them; 20 px of
  indent per level; the disclosure chevron a 12 px SVG (`m9 6 6 6-6 6`,
  stroke-width 2.5) turning 90° when open; the row's ⋮ button 28 × 28 px,
  radius 6 px.
- Details pane: 320 px wide, padding 20 px, 16 px between its blocks, the
  title 18 px bold, the details 13 px in a two-column grid (10 px between rows,
  12 px between columns), the **Open** button at least 44 px high, padding
  `0 16px`, radius 8 px, filled with accent, white semibold text.
- Activity panel: its row at least 36 px high, padding `0 8px 0 16px`, 13 px;
  the progress bar 96 × 4 px, radius 2 px, track in border colour and fill in
  accent; the `<n> failed` badge 12 px semibold, padding `2px 8px`, radius
  10 px, refused on its ground; the **?** a 28 px circle with a `#d5d9df`
  border. In the expanded panel, the group headings 11 px semibold, uppercase,
  letter-spacing 0.04em, in dim; entries at least 32 px high; the outcome dot
  6 px.
- Actions menu: 232 px wide, padding 6 px, radius 8 px, 1 px border, shadow
  `0 12px 32px rgba(0, 0, 0, 0.16)`; its title 12 px in dim; items at least
  34 px high, padding `0 10px`, radius 6 px, 13 px.
- Dialogs: `min(480px, 100% - 32px)` wide, padding 24 px, radius 12 px, 14 px
  between blocks, shadow `0 24px 64px rgba(0, 0, 0, 0.24)`.
- Font: `Inter, system-ui, sans-serif`, 14 px base.

Where the explorer has an element these values do not cover, give it the
nearest role above rather than a new colour. Out of scope: the search box (not
designed yet), a dropdown arrow on the Library's name (nothing to switch to
yet), and the dark set and its switch.

The end-to-end journeys (`frontend/packages/apps/e2e/`, run by `make e2e-it` and `make e2e-electron`, and in CI) must keep passing: when a change moves something a journey relies on (a role, a label, a row it counts), update the journey in this same change so it still proves what it proved.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A unit test covers which icon a row draws: a folder, an image file the
  reader can show, and another file.
- [x] A test reads the explorer's component sources
  (`frontend/packages/apps/web/src/*.tsx`, tests excluded) and fails on any
  colour literal (a hex code, `rgb(`/`rgba(`) outside `theme.ts`, so every
  colour, shadows included, goes through a name.

### Before merge (verified outside the check command)

- [x] On `make local`, the agent compares the tree with the details pane, the
  header, the Activity panel (collapsed and expanded), the actions menu, a
  dialog and the reader sheet against the values above, and records in the PR
  what it saw, with any difference it left and why.
