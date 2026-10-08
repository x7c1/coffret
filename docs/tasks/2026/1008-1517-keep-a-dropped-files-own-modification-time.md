---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -q "toLocaleString()" frontend/packages/apps/web/src/humanize.ts'
assignee: null
branch: task/1008-1517-keep-a-dropped-files-own-modification-time
created_at: 2026-10-08T06:17:54Z
updated_at: 2026-10-08T07:20:29Z
---

# feat(explorer): keep a dropped file's own modification time, record no birth time for it, and show times as RFC 3339

## Overview

A file dropped onto the explorer arrives as its name and its bytes only
(`frontend/packages/gateway/api/src/upload.ts`, `addFiles`). The server writes
it into the mapped folder (`backend/crates/apps/coffret-server/src/routes/upload/receive.rs`)
and the file's modification time becomes the moment of the drop. The scan that
follows (`coffret-local-fs/src/unix_mapped_roots/list_folder.rs`) reads that
mtime, and on filesystems that report a birth time it also reads the receiving
file's btime, and both are recorded on the Entry as its `original_mtime` /
`original_btime` (spec: FM-9). A book scanned in 2015 and dropped today is
recorded as made today, and the btime recorded is the scratch file's, not the
original's.

The concepts say the Entry's original times are those of the file that was
stored (`docs/concepts/container/entry/README.md`), and an absent time is
recorded as absent rather than filled in (FM-9, FM-15).

**1. Send and keep the mtime.** The explorer sends each dropped file's
`File.lastModified` (milliseconds since the epoch) with its part — as a
per-part field the server can read before it writes the file, chosen to fit
the existing multipart reading. The server sets the written file's
modification time to it (`std::fs::File::set_times` / `FileTimes`, which works
the same on Linux, APFS and exFAT) after the bytes are written and before the
file is renamed into place. A missing or unreadable value leaves the mtime as
written and is not an error. Define in the spec and test how milliseconds map
to FM-9's seconds (truncate toward negative infinity, so a time before 1970
stays before it) and that negative times are accepted.

**2. Record no birth time for a dropped file.** A browser cannot tell the
server a file's creation time, so the birth time the receiving filesystem
reports for a dropped file is not the original's. Make the Entry created from
a dropped file carry no `original_btime`. Find the narrowest place to do this:
for example, the drop records that it placed this path with no known birth
time, and the scan or the commit that turns it into an Entry uses that record
instead of the stat's btime. A file put into the mapped folder any other way
(copied by the person, synced from the command line) keeps today's behaviour.
State the rule in the spec where FM-9's times are defined or where the drop is
specified (EP-11), and in the Entry concept if it describes where the times
come from.

**3. Show times as RFC 3339.** `frontend/packages/apps/web/src/humanize.ts`
`time()` formats with `toLocaleString()`, so the explorer shows a browser's
locale format (e.g. 10/8/2026, 2:03:00 PM). Show RFC 3339 with the device's own
offset instead, to the second: `2026-10-08T14:03:00+09:00`. Keep the function's
handling of a missing time.

Out of scope: correcting an Entry's time by hand, searching or sorting by time,
and changing what a sync records for files placed outside the explorer.

## Acceptance criteria

### Automated (pipeline-verified)
- [x] The upload sends each file's `lastModified`, and the server sets the written file's mtime from it, with a route test that a dropped file's Entry carries the sent time in seconds
- [x] Milliseconds before 1970 map to the earlier second, with a unit test
- [x] An Entry created from a dropped file carries no birth time even on a filesystem that reports one, with a test; a file placed in the mapped folder otherwise keeps its birth time
- [x] A part without a modification time is stored as before, with a test
- [x] The spec states the drop's time rules, and `make spec-citations spec-rule-ids` passes
- [x] `humanize.time` renders RFC 3339 with the local offset, with a unit test, and no `toLocaleString()` remains there
- [x] `make check` passes

### Before merge (verified outside the check command)
- [ ] Needs a person: under `make desktop-dev`, dropping a years-old file onto a folder shows its original date in the explorer's list
