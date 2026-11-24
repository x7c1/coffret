---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, spec-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check"
assignee: null
branch: task/1124-1542-fix-the-activity-offer-rows-filter-name-limit-and-root-scan-read
created_at: 2026-11-24T06:42:28Z
updated_at: 2026-11-24T07:41:41Z
---

# fix: give the Activity offer rows room, stop the filter's name at its limit, and scan only a root mapping's own rows

## Overview

Three small defects, each found while reviewing an earlier change and left for later. They do not share one rule; they are put in one change because each is a few lines, and none depends on another. Line numbers are from when the sites were listed; confirm each before changing it.

- **The Activity panel's bottom bar crowds an offer of several lines.** `frontend/packages/apps/web/src/ActivityPanel.tsx`: `ROW` (around line 657) is built for one line — `minHeight: 36` and no padding above or below. While Drive's permission has run out, the row holds an offer of two or three stacked blocks (the catch-up sentence, the sentence saying the permission ran out, the waiting sentence with its "no tab opened?" link), and they touch the row's top and bottom edges. Give a row whose content is taller than one line room above and below (a block padding on the row, or a wrapper around the offer), so that the one-line row keeps its 36px height and nothing in it moves: the row must not shake when its summary changes, and the collapsed row's parts on the right must stay where they are. Change no wording, order, button or link.
- **The explorer filters by a name longer than the server keeps.** The server refuses to keep a filter whose name is longer than 1,000 characters with `400` (spec: LA-43, `backend/crates/apps/coffret-server/src/routes/filter_settings/`). The explorer (`frontend/packages/apps/web/src/useKeptFilter.ts` and the search field in the header) does not limit the name, so once it is longer, every later save of the filter's settings fails and is only written to the console until the name is shortened. Stop the name at the limit where it is typed or pasted, so the explorer never asks to keep what the server refuses. Count the length the way the server does (confirm in the route whether it counts characters, UTF-16 units or bytes, and match it), and keep the limit in one named constant that says which rule it comes from.
- **A sync's scan reads every present row of the device for a Library-root mapping.** `backend/crates/domain/coffret-usecase/src/sync/scan/deletions.rs`: for a root mapping whose scope holds the root, `reads` asks `present_under(None)` and drops the rows that other mappings represent afterwards. The root mapping represents *the remainder* (spec: EP-9), so what it needs is the present rows outside the top-level names represented elsewhere, and, for a scope narrower than the whole Library, only those the scope covers. Read only those: add a narrow Index port method where none answers it (with a default implementation on the trait that the in-memory index can keep, and a conformance case run against both the in-memory and the SQLite index), or bound the existing read. The scan must report the same deletions as before for every arrangement of mappings and scopes; the existing tests pin that and must keep passing unchanged. If the wider read turns out to be needed for correctness in some arrangement, keep it there and say why in a comment and in the PR description.

Do not add or remove behavioural rules in `docs/spec/`. Where a rule already describes one of these (LA-43's limit, EP-9, EP-10), keep it true. Change nothing under `.github/`.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes
- [x] A web test pins that an Activity row holding an offer of several lines has room above and below it, and that a one-line row keeps its height
- [x] A web test pins that a name longer than the limit cannot be entered into the filter, typed or pasted, and that the settings the explorer asks to keep never hold a longer one
- [x] The scan no longer reads every present row of the device for a root mapping (or the PR description says where it still must and why), and each new Index port method has a conformance case run against both indexes

### Before merge (verified outside the check command)

- [x] The PR description says, for each of the three, what changed and how it was checked, and says that the look of the offer row while Drive's permission has run out was checked by tests only
