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
branch: task/1018-0136-detect-local-renames-during-sync
created_at: 2026-10-17T16:36:40Z
updated_at: 2026-10-17T17:19:42Z
---

# feat(sync): carry a file renamed inside a mapped folder into the Library as a rename

## Overview

`coffret rename` commits a rename without uploading anything. A file the user renames on disk is still read by sync as a disappearance plus a new file: the new file is uploaded again as a new Entry, and the old Entry stays in the Library, reported as deleted locally. This change lets sync recognize the narrow, unambiguous case and commit it as a rename instead.

### 1. The rule (EP-10, EP-9's scan wording, Library concept)

Within one sync's scan, pair a disappearance with an appearance as a rename only when **all** hold:

- The disappeared file is one this device materialized: it has a materialization record (EP-10) for a current Entry, not awaiting relocation, and its file is gone from where it stood.
- The appeared file is new to the scan: no Entry at its Entry Path, and no record for it. It lies under the **same mapped root** as the disappeared one.
- Its size and content hash equal the record's Entry's size and hash. The scan already hashes a new file before uploading it, so nothing extra is read.
- The pairing is **one-to-one** within the scan: exactly one disappeared candidate and exactly one appeared candidate share that (size, hash). Any ambiguity — two disappeared Entries with the same content, two new files with it, or one of each plus another — pairs nothing, and every file involved is handled as today (uploaded as new, or reported deleted locally).

Pairing happens within the scan's own scope ("one scan's scope", not "a full sync"). A disappearance that finds no partner keeps its record and is reported deleted locally exactly as today, so a later scan whose scope covers the partner can still pair them. Do not change what triggers a sync or its scope.

### 2. What a pair becomes

- One `renames` element per pair, in the same batch the sync commits: the Entry (Container ID, entry number) gets the appeared file's Entry Path. The batch carries `required_schema` like any record with renames, and is rebased like any batch (rules 1–8). A conflict refuses the batch as today.
- No upload and no Container for a paired file. The new file is not selected for addition, and the old Entry is not reported deleted locally.
- The record is re-keyed to the new path after the commit, with its observation unchanged. The file already stands at the Entry's new path, so nothing waits for relocation.
- Every rule a rename planned with `coffret rename` must pass applies here too (destination taken, file-and-folder, reserved namespace, the FM-15 rename-set check). A pair whose rename would be refused is not paired; its files are handled as today, and the sync reports why with a typed finding.
- A pair appears in the sync's answer and findings, as `renamed <old> → <new>` (text) and as a `renamed` count with a finding per pair (JSON). It is never silent.

### 3. Spec and concepts

- EP-10: the detection rule and what a pair becomes.
- EP-9: the scan wording.
- Library concept: lift the Technical Constraint "does not infer a rename from a local file being renamed", and describe the narrow rule plus the collocation (*a sync carries a local rename into the Library*).
- Journal, Catalog: a sync may write `renames`.
- Keep the vocabulary: *rename*, *relocate*, *discard*, never *move*.

### Tests

Run them against both Index implementations where the sync conformance suite does.

- A file renamed within a mapped root becomes one rename: zero Container puts, the record re-keyed, and nothing reported deleted locally.
- The same file renamed into a sub-folder, and a whole folder of files renamed (each file pairs one-to-one).
- Ambiguity pairs nothing:
  - two identical files where one disappeared
  - one disappeared and two identical new files
  - a disappeared file with no record (never materialized)
  - an appeared file with different content
  - a rename across two mapped roots
- An edited-and-renamed file (hash differs) is uploaded as new, and the old Entry is reported deleted locally, as today.
- A pair whose rename is refused (destination rule) falls back with a finding.
- A detected rename racing another device's commit is rebased or conflicts per the existing rules.
- Another device fetches after the detected rename and relocates its file without reading a Container.

### Out of scope

Changing the sync scope of drops, explorer UI, restore/salvage wording.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A file renamed inside one mapped root, whose content and size match its record's Entry and which pairs one-to-one within the scan, is committed as a rename with no Container upload (tests count puts).
- [x] Every ambiguous or ineligible case pairs nothing and behaves exactly as before (tests).
- [x] A detected rename is checked against the same rules as `coffret rename`. A refused pair falls back with a typed finding (tests).
- [x] The sync's answer reports each detected rename, in text and JSON (tests).
- [x] EP-9, EP-10 and the Library, Journal and Catalog concepts describe the detection (spec checks pass).
