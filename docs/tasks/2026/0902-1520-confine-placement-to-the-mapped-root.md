---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && grep -rqi 'symlink' backend/crates/domain/coffret-usecase/tests && grep -rqi 'symlink' backend/crates/apps/coffret-device/src/add"
assignee: null
branch: task/0902-1520-confine-placement-to-the-mapped-root
created_at: 2026-09-02T06:55:00Z
updated_at: 2026-09-02T11:30:00Z
---

# fix(fetch): confine placement to the mapped root

## Overview

The scan side already treats symlinks with suspicion: EP-8 reads a mapped
folder without following them. The write side does not keep the same
discipline. `fetch/translate.rs` turns an Entry Path into
`local_root.join(components...)` without asking what the intermediate
components are on disk; `fetch/select.rs` checks `symlink_metadata` only on
the final target; and then `fetch/placement.rs` and
`coffret-device/src/add/incoming_file.rs` run `create_dir_all(parent)`,
create a scratch file inside the parent, and rename onto the destination —
every one of which lets the OS follow a symlinked directory on the way
down.

That breaks the Library's own promise in a concrete way. Entry Paths come
from other enrolled devices, and the same path materializes onto different
filesystem shapes on each device. Commit an Entry `link/authorized_keys`
from a device where `link` is an ordinary directory; on a device where the
mapped root happens to contain `link -> ~/.ssh`, fetch sees that the final
target does not exist, decides the spot is free, and replaces
`~/.ssh/authorized_keys`. No malice is required — an unlucky filesystem
shape is enough — and the upload route reaches the same placement code
through `IncomingFile`. EP-4 and EP-11 say a path that cannot be
materialized is refused explicitly and bytes are placed only where the
device can vouch for them; writing through a symlink out of the root is
exactly the thing those rules exist to refuse.

Make placement descend the way scan reads:

1. **Component-wise, no-follow descent.** Resolve the mapped root once
   (the root itself is the user's configured place; what it points at is
   theirs to choose), then walk each Entry Path component from that root
   refusing to pass through anything that is not a real directory —
   a symlink met anywhere on the way down, existing or racing into place,
   means the path is unmaterializable. Prefer holding a directory handle
   and descending with no-follow semantics (`openat`-style, `O_NOFOLLOW` /
   `O_DIRECTORY`) over a one-shot pre-check of the whole path, so the
   answer cannot go stale between the check and the write; if any step
   must fall back to path-based inspection, say why in a comment at that
   step. Directory creation, scratch-file creation, and the final rename
   must all happen relative to the verified descent, not to a re-joined
   absolute path.
2. **Both writers, one discipline.** `fetch/placement.rs` and
   `add/incoming_file.rs` (which the upload route uses) must share the
   confinement rather than duplicating two slightly different versions of
   it. Put the descent where both can own it and keep the existing
   refusal shapes: a fetch that meets a blocked path while selecting
   surfaces that Entry as a finding and **keeps going** — one symlinked
   folder must not turn a whole fetch run into a failure, in line with
   the product's own findings-over-failures posture (a descent that goes
   wrong mid-write, after selection said the place was sound, may still
   fail the run — that is a race, not a shape). An upload is refused
   with the existing explicit-refusal vocabulary. Carry the blocked
   component's name into the human-facing message (never the log), the
   way unavailable-root findings already do — the person's next step is
   `ls -l` on that one folder, so name it.
3. **Tests that prove the fence, not just exercise it.** Under a scratch
   mapped root, on the paths that exist today: a parent component that is
   a symlink to a directory outside the root (fetch and upload both
   refuse, and the file outside the root is byte-for-byte unchanged
   afterwards); a symlink deeper in the chain, not just at the first
   component; a symlink pointing *inside* the root (still refused — the
   canonical location is the one the Library names, and a second name for
   it is not); and the ordinary all-real-directories case still placing
   files exactly as before. These land in the usecase fetch tests and the
   device-side add tests (today neither mentions symlinks at all — the
   `check_command` greps pin that).

Update the doc comments that currently describe placement as "join and
create": the confinement is now part of what placement means, and EP-4 /
EP-11 are the rules to cite.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Fetch refuses to materialize an Entry Path whose on-disk parent
      chain under the mapped root contains a symlink, at any depth,
      whether it points inside or outside the root; a test proves the
      outside-the-root target is byte-for-byte unchanged after the
      refusal.
- [x] The upload path (`IncomingFile`) enforces the same confinement with
      its own test, and both writers go through one shared descent rather
      than two copies of the check.
- [x] A folder fetch that meets one blocked path still places every
      other Entry and reports the blocked one as a finding naming the
      blocking component; a test proves an unrelated Entry lands while
      the blocked one is surfaced.
- [x] Placement's directory creation, scratch file, and rename operate
      relative to the verified descent (no re-joined absolute path between
      verification and write); where a platform truly cannot express
      no-follow semantics for a step, the code says so at that step.
- [x] The ordinary case is unchanged: existing fetch and upload tests
      (including the E2E journeys) pass without modification to their
      expectations.

## Out of scope

- Request, file, and staging-space limits — nothing here changes them.
- What the server logs and how errors are rendered — refusals here reuse
  the existing shapes.
- Windows semantics (junctions, reparse points): the product targets
  Linux and macOS; the descent should be written against Unix no-follow
  primitives, not a portability layer for platforms the product does not
  run on.
- Changing EP-8's scan behavior — scan already has the right discipline;
  this task brings the write side up to it.
