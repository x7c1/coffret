---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "part_bytes" frontend/packages/gateway/api/src/upload-budget.json && grep -q "parts" frontend/packages/gateway/api/src/upload-budget.json'
assignee: null
branch: task/1008-1108-refuse-a-drop-the-server-cannot-take-before-sending-it
created_at: 2026-10-08T02:08:29Z
updated_at: 2026-10-08T02:51:42Z
---

# feat(explorer): refuse a drop the server cannot take before sending it, and say how to add it instead

## Overview

The server takes one drop within three budgets (spec: LA-9;
`backend/crates/apps/coffret-server/src/allowance.rs`): 64 GiB per request,
1 GiB per part (one part is one file), and 4096 parts per request. The
explorer checks only the first before sending
(`frontend/packages/gateway/api/src/upload.ts`, `addFiles`, reading
`upload-budget.json`). A drop holding a file over 1 GiB, or more than 4096
files, is sent anyway: the server refuses it part way through, after some
files may already have landed in the folder, and a browser commonly reports
that refusal as a broken transfer. Neither answer says what to do instead.

The Library itself has no size limit on an Entry (a video of several
gigabytes is a valid file to keep). What cannot carry it is this route, so the
explorer should say so and point to the way that can: the command line. A
file copied into the mapped folder on this device is carried into the
Library by `coffret sync --library <name>`; the command line is built from
this repository (`docs/guides/install/README.md`).

**1. Publish all three budgets to the frontend.** Extend whatever writes
`upload-budget.json` (it is generated or pinned from the server's
`Allowance` — follow how `request_bytes` gets there and keep one source of
truth) with the per-part byte budget and the part count, under keys such as
`part_bytes` and `parts`. A test on the server side keeps the JSON equal to
`Allowance`'s values, the way `request_bytes` is kept.

**2. Refuse before sending.** In `addFiles` (or where a drop is turned into a
request), refuse a drop whose any one file is larger than the part budget, or
whose file count passes the part budget, before anything is sent — as the
request budget is refused today, with nothing written.

**3. Say why and what to do.** Following the explorer's rule that every
refusal pairs its reason with an action, the sentence a person sees names the
cause and the alternative, for example:
- a file over the part budget: "`<name>` is 3.2 GB, more than the 1 GB one
  file can be when dropped here. Copy it into this folder on the device it is
  mapped to and run `coffret sync --library <library>`."
- more files than the part budget: "this drop holds 5,210 files, more than the
  4,096 one drop can carry. Drop its subfolders one at a time, or copy it
  into the mapped folder and run `coffret sync --library <library>`."
Use the Library's name and sizes the explorer already has; format sizes the
way the explorer formats them elsewhere. Where the request budget is passed,
give it the same shape (reason and action) instead of the current sentence.

Out of scope: a chunked upload route, and any change to the server's budgets.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] `upload-budget.json` carries the per-part byte budget and the part count, and a server-side test holds them equal to `Allowance`
- [x] A drop with one file over the part budget is refused before any request is sent, with a sentence naming the file, the limit and the command-line alternative (unit test)
- [x] A drop with more files than the part budget is refused before any request is sent, with a sentence naming the count, the limit and both alternatives (unit test)
- [x] A drop within all three budgets is sent as before (existing tests)
- [x] `make check` passes

### Before merge (verified outside the check command)
- [ ] Needs a person: in the explorer under `make desktop-dev`, dropping a file larger than 1 GB shows the refusal sentence and writes nothing into the folder
