---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1014-1600-name-the-files-a-drop-replaces-and-close-four-small-gaps-in-the-explorer
created_at: 2026-10-14T07:01:46Z
updated_at: 2026-10-14T07:37:51Z
---

# fix(explorer): name the files a drop replaces, and close four small gaps in reconnect, trash and the list

## Overview

Five small findings from earlier reviews and from using the explorer. Each is
local; together they touch the explorer's drop question and list, the
reconnect link, and the server's handling of the desktop's trash folder.

### 1. A drop into a folder of the same name says which files it replaces

Dropping a folder onto the Library where a folder of that name already exists
sends its files to the same paths. A path that holds a one-file Entry is
updated in place (replaced); one inside a Pack is refused. Today the question
before sending (`packChoice.ts`, `PackConfirm.tsx`) only says, in general
terms, that a file with the same name "replaces it (or is refused, where a Pack
holds it)". Keep that rule. Before sending, work out from what the explorer
already knows of the landing folder (the listing it has, or one more listing
request) which dropped paths already hold a one-file Entry and which are inside
a Pack, and say it in the question: how many files will replace existing ones,
naming the first few (for example up to five, then "and N more"), and how many
will be refused because a Pack holds them. Where no path collides, the
question says nothing more than it does now. Do not add a "skip existing"
choice. Tests in `packChoice.test.ts` / `PackConfirm.test.tsx` cover a drop
with no collision, with replacements only, and with both replacements and
Pack refusals.

### 2. The reconnect link follows a new consent flow

The "open the consent page" link (`App.tsx` `consentPage`, `reconnect.ts`
`consentLink`) can keep an old URL when, between two polls, one consent flow
ends and another begins: the state stays `waiting` → `waiting`, so nothing
updates the link. Make the link follow the URL the server currently reports
for the waiting flow (compare the URL, or a flow identifier if the answer has
one, rather than the state alone). Add a test for `waiting` (URL A) →
`waiting` (URL B) showing the link becomes B.

### 3. A drop onto the desktop's trash folder is refused in its own words

A drop whose path is inside the desktop's trash folder (`.Trash`) is refused
with the `ReservedComponent` sentence, which talks about `.coffret`
(`coffret-server/src/api_error/from_error.rs` ~429). Give that case its own
error variant where the refusal is raised, its own sentence (the folder is the
desktop's trash, which coffret keeps out of the Library), and its own sample in
`contract/refusals.json`, following how the other refusals are added. The
explorer hides the folder, so the path is hard to reach; a route test is
enough.

### 4. `added_at` keeps the trash folder out

`coffret-device/src/add/added_at.rs` (the single-path form of
`added_locally`) does not exclude the trash folder, so `.Trash/x.jpg`, if it
exists, opens as a local file. Apply the same exclusion `added_locally`
applies, and add a test beside the existing ones.

### 5. The list's selection row and drop band do not hide the top row

In `FileList.tsx` the selection row and the drop band are drawn absolutely
positioned just below the sticky block, so while the list is scrolled they
cover roughly the top 25px of the first visible row; a ⋮ menu opened on the
last row can likewise be clipped at the bottom of the scroll area. Lay them out
so they take their own space (for example inside the sticky block) or reserve
the space they cover, and let the last row's menu open upwards when there is
no room below. Keep the existing tests passing and add one that the first row
is not overlapped where the test environment can tell (otherwise state in the
code why it cannot be asserted).

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The drop question names how many dropped files replace one-file Entries
      (with the first few names) and how many a Pack refuses; tests cover no
      collision, replacements only, and both
- [x] The consent link follows a new URL across `waiting` → `waiting`
      (test)
- [x] A drop into `.Trash` is refused with its own variant, sentence and
      `contract/refusals.json` sample (route test)
- [x] `added_at` does not open a path inside the trash folder (test)
- [x] The selection row and drop band no longer overlap the first row; the
      last row's menu opens upwards when there is no room below
- [x] `make check` passes

### Before merge (verified outside the check command)

- [ ] On the agent's Library (Playwright Chromium): dropping a folder whose
      name matches an existing folder shows the replacement count and names
      before anything is sent; scrolling the list leaves the first row fully
      visible under the selection row (screenshot; the look is the user's to
      judge)
