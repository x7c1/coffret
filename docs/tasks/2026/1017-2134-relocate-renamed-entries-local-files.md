---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check'
assignee: null
branch: task/1017-2134-relocate-renamed-entries-local-files
created_at: 2026-10-17T12:34:46Z
updated_at: 2026-10-17T14:10:27Z
---

# feat(local): relocate a renamed Entry's local file, and tie materialization records to the Entry they record

## Overview

When a rename is applied, this device's file for the renamed Entry stays at the path it was placed, and its materialization record waits for relocation (`LocalEntry::relocate_to`, EP-10). Nothing moves the file yet, so the device keeps an old-named file the Library no longer names, and the Entry is remote at its new path. This change relocates such files on disk and closes the identity gap that makes waiting rows fragile.

### 1. Materialization records name the Entry, not only the path

A materialization record today says "the file at path P is this device's copy of the Entry at P". After renames and removals, P can come to hold a *different* Entry, and the record is then read as a copy of that other Entry (a scan would offer an edit of the file as an update of the other Entry). Known cases: a departed row whose path a later rename gives to another Entry; adopting a Snapshot that both swapped and removed Entries.

Tie each record to the Entry it records — (Container ID, entry number), kept current as replacements (update, freeze absorption, rebuild) give the Entry a new Container and number. Wherever the catalog places a different Entry at a record's path, the record does not claim it: it is forgotten (or ends its wait) exactly as EP-10's existing rule for released waits does, and the file is judged where it stands (EP-15). Upgrade the SQLite layout and fill the new columns from the catalog for rows whose path still names their Entry; rows that cannot be matched are dropped with a finding, never guessed.

### 2. Relocate

A run that holds this device's mapped roots (decide which — at least `sync` and `fetch`, and the server's equivalents; justify the choice in the PR) relocates every row awaiting relocation before it scans or places anything:

- Rename the file on disk from its old path to the Entry's current path with one filesystem rename, creating missing parent folders, and only then re-key the record to the new path (decision: the record changes after the file, so an interrupted relocation leaves the old record with the file still at the old path, and the next run retries; it never leaves a record pointing at a path with no file).
- **Refuse, report, and keep waiting** (each a typed finding naming the reason; the file stays where it is and is never overwritten): the mapped root holding either path is unavailable (EP-12); the file was edited since it was placed (its size/mtime/hash no longer matches the record's observation); the destination already exists and is not this device's placed copy of the same Entry; the two paths lie under different mapped roots or on different filesystems (no copy-and-delete fallback); either path crosses a symbolic link (EP-8).
- **Destination already holds this Entry** (a fetch placed it at the new path while the old file waited): the old file is redundant — discard it to the desktop's trash when unedited (EP-15's discard), keep it and report when edited; the record for the new path is the one that stands.
- Empty folders left behind by relocation are removed as the existing `remove` collocation does after discards.
- Relocation never uploads, re-encrypts or commits anything.

### 3. Spec and concepts

EP-10 (relocation, refusal reasons, the destination-already-placed case, interruption), EP-12, the Library concept (relocate collocation, the awaiting-relocation state ending), Index concept (records tied to the Entry), and the Entry Path concept.

### Tests

- Relocation of an unedited file inside a mapped root (single file, Pack Entry, folder's worth, swap, rename back), including creating parent folders and removing emptied ones.
- Each refusal reason leaves file and record unchanged and reports it; a later run after the cause is cleared relocates.
- Interruption: before the filesystem rename, after it but before the record update — the rerun converges with no duplicate upload, no overwrite, no lost file (use the existing fault-injecting filesystem fakes).
- Destination already placed: unedited old copy discarded, edited one kept and reported.
- The identity cases from §1: a departed row whose path a rename gives to another Entry, and adopting a Snapshot with swap + removal — neither record is read as the other Entry's copy (no update proposed).
- Layout upgrade fills record identities and drops unmatched rows with a finding.

### Out of scope

The rename operation (CLI) and local rename detection.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Materialization records carry the Entry they record and never claim a different Entry at their path; the two identity cases are covered by Index conformance and sync tests against both Index implementations.
- [x] A sync (and each other run chosen to relocate) relocates waiting files with one filesystem rename, then re-keys the record; tests cover the shapes listed above.
- [x] Each refusal reason is a typed finding with file and record unchanged, and a later run relocates once the cause is gone (tests).
- [x] Interrupted relocation converges on rerun without overwriting, losing, or re-uploading a file (tests with fault injection).
- [x] The destination-already-placed case discards an unedited old copy and keeps and reports an edited one (tests).
- [x] The SQLite layout upgrade fills identities and drops unmatched rows with a finding (test).
- [x] EP-10 / EP-12 and the Library, Index and Entry Path concepts describe relocation (spec checks pass).
