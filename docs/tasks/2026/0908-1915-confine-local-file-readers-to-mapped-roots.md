---
status: "completed"
pipeline_phase: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/0908-1915-confine-local-file-readers-to-mapped-roots
created_at: 2026-09-08T19:15:03Z
updated_at: "2026-09-09T07:20:50Z"
---

# fix: confine local file readers to mapped roots

## Overview

The file route opens translated paths again after deciding whether a file is
present, locally added, or fetched. A directory or final name replaced with a
symbolic link between those steps can make the server serve another file. The
local filesystem gateway also opens scan sources directly by path, leaving the
same gap between enumeration and the hashing or encoding reads.

Acquire regular-file readers through the local filesystem capability by
descending from the configured mapped root without following links below it.
Keep the resulting handle through streaming, and obtain the response length
from that handle. The user-configured root may still resolve through a link;
its descendants and final filename must not. Refuse nonregular files without
blocking on a FIFO. Do not buffer entire files or create missing roots while
reading. Preserve missing-file, unmaterializable-path, and conflict behavior.

Carry the mapped root and validated relative location through the scan source
and device interfaces instead of inferring a root from an arbitrary absolute
path. Inspect folder enumeration for the same parent-link race and keep its
actual no-follow guarantee aligned with the source reader. Reuse the existing
descriptor descent where practical, retaining the gateway boundary and
`forbid(unsafe_code)` in the use-case layer. The server must continue to reach
the filesystem through coffret-device, not depend directly on a gateway.

Cover all three file-route sources and source reads used for sync/freeze.
Update the Entry Path concept and EP-8/EP-11 to describe the actual read and
write confinement boundaries, including the deliberately followed configured
root. Correct related stale descriptions in touched files. Keep changes to
error types structured and preserve their safe diagnostic rendering.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Gateway tests prove ordinary and missing-file reads, refusal of parent
      and final symlinks and nonregular files, and no root creation during reads.
- [x] A deterministic test opens a reader, replaces its path, and proves the
      reader retains the originally opened bytes and handle-derived length.
- [x] Source-read tests prove that a symlink substituted after enumeration
      cannot be followed during hashing or encoding; the fake and real gateway
      share the revised capability contract where representable.
- [x] Server tests exercise present, locally added, and newly fetched files
      through the confined reader, retain successful streaming and length
      behavior, and refuse reads through planted links without returning the
      outside file's bytes.
- [x] Existing scan, sync, freeze, fetch, and routing regressions pass, and the
      dependency check continues to prohibit server-to-gateway dependencies.

## Out of scope

Persistent root markers, volume identity, database migrations, mount-crossing
policy, and changes to the file-route locking lifecycle are separate changes.
Do not redesign the explorer or move unrelated error and test modules.
