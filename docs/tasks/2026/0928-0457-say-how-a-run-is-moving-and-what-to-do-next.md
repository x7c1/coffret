---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqF "renewed and cached, sealed" --exclude-dir=target backend/crates/apps/coffret-cli/ && ! grep -rqF "rather than sits in one" --exclude-dir=target backend/crates/apps/coffret-device/'
assignee: null
branch: task/0928-0457-say-how-a-run-is-moving-and-what-to-do-next
created_at: 2026-09-28T04:57:29Z
updated_at: 2026-09-28T05:14:36Z
---

# fix(cli): say how a run is moving and what to do next, without holding the run up to say it

## Overview

The CLI's output should tell a person how a run is moving and, when it stops, what to do next, in words about their Library rather than about coffret's mechanism — and saying so must never slow the run. Four places fall short of that.

1. **`fetch --entry` is silent while it works.** A folder fetch reports progress on stderr (`coffret-cli/src/fetch.rs`, around lines 44-47, `Reporting::to_stderr(Units::Fetching)`); a one-Entry fetch passes no progress at all (`fetch.rs:57`, `run_fetch_entry`), and `FetchEntryRequest` (`coffret-usecase/src/fetch/entry_request.rs`, around lines 18-40) has nowhere to take one, although the folder request does (`fetch/fetch_request.rs`, `progress` and `watched_by`, around lines 55 and 95). A range read out of a Pack can be megabytes, so the wait can be long. Thread a progress sink through `FetchEntryRequest` the way the folder request does (`coffret-device/src/run_fetch_entry.rs` and the device's `fetch_entry`), have the use case report the ranged read's steps, and have the CLI pass `Reporting::to_stderr(Units::Fetching)` and finish it before printing the summary, as the folder branch does.
2. **An unmapped Entry's refusal does not say how to get past it.** A one-Entry fetch whose path no mapping reaches ends in the use case's `UnmappedEntryPath` refusal (`coffret-usecase/src/fetch/fetch_error.rs`, around lines 535-539: "no mapping of this device says where the Entry … would go"). The folder branch, meeting the same state as `mappings == 0`, adds a line naming `coffret map` (`report::nothing_mapped`, `Unmapped::NowhereForTheLibraryToGo`). Have the CLI add the same next step when a one-Entry fetch ends in that refusal. The command name belongs to the CLI, not to the use case's sentence, so the use case's wording stays; the server's `api_error/refused_root.rs` is the precedent for a shell naming the gesture over a lower layer's refusal.
3. **The CLI's `Progress` writes to stderr on the runtime's thread.** `Reporting::step` (`coffret-cli/src/progress.rs`, around lines 168-196) calls `write_all` and `flush` from inside the flow, on a tokio worker thread. The contract it implements says reporting must not hold a run up (`coffret-usecase/src/progress.rs`, around line 119); a slow reader on a pipe, or a slow terminal over ssh, blocks that write and the transfer with it. Make `step` only record the latest `Step` in a shared cell and move the drawing to a separate thread that takes the latest one at an interval and writes it; move the rules `finish` and `Drop` follow today (around lines 122-131 and 198-220 — clear on success, leave the line on failure) to that drawing side. Pin the property with a unit test that injects a `Write` that never returns and asserts `step` returns at once (`Reporting::new` already takes its `out`). The rendered output and its wording stay as they are.
4. **`authorize` answers in mechanism words, and one `NameDefect` sentence breaks the pattern of its siblings.** The success line (`coffret-cli/src/authorize.rs`, around lines 47-50) says "The grant is renewed and cached, sealed, for the account on this device"; what a person needs is that this device can reach Storage as that account again and that every Library referencing the account uses it from its next run. `NameDefect::Relative`'s sentence (`coffret-device/src/error/name_defect.rs:32`) is "it names a directory rather than sits in one", where every sibling is "it" plus one verb phrase ("it is empty", "it holds a path separator"); say, for example, "it is `.` or `..`, which name a directory rather than one inside it". Use the account vocabulary the concept documents register (grant, account) and check the new line against them.

Guards:

- The progress phases' wording, and what `Reporting` prints on a terminal and on a pipe, stay as they are; item 3 changes which thread writes, not what is written.
- The use case's `UnmappedEntryPath` sentence stays; item 2 adds the CLI's line after it.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `FetchEntryRequest` carries a progress sink and the CLI's one-Entry branch passes `Reporting` to it (`make check`, with a test in the use case's fetch conformance or unit tests that a ranged Entry fetch reports its steps)
- [x] A `coffret-cli` test asserts that a one-Entry fetch ending in `UnmappedEntryPath` prints a line naming `coffret map` (`make check`)
- [x] A unit test injects a never-returning `Write` into `Reporting` and asserts that `step` returns without waiting on it (`make check`)
- [x] "renewed and cached, sealed" no longer appears under `coffret-cli` (grep gate), and a test pins the new `authorize` success line (`make check`)
- [x] "rather than sits in one" no longer appears under `coffret-device` (grep gate)
- [x] `make check` passes
