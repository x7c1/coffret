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
branch: task/1017-2328-rename-entries-and-folders
created_at: 2026-10-17T14:28:49Z
updated_at: 2026-10-17T15:20:40Z
---

# feat(cli): rename an Entry or a folder of Entries without uploading its content

## Overview

Everything a rename needs below the operation is in place: Journal records carry `renames` by (Container ID, entry number) (FM-15), the Index applies them (RV-5), a batch carrying them is rebased onto a newer head (CP-4, CP-7, EP-7), fetch reads by entry number and places at the current path (EP-11), and sync and fetch relocate a device's waiting files (EP-10). Nothing writes a rename yet. This change adds the operation, so renaming a file or a folder in the Library commits one Journal record and uploads no Container.

### 1. Old builds must not silently drop renames

FM-15's maps are forward-open: a reader ignores fields it does not know. A build that predates `renames` would accept a record carrying them, apply its removals and additions, ignore its renames, and from then on hold a catalog that disagrees with every other device's, without saying so. Before any writer emits `renames`, add a must-understand rule:

- A Journal record may carry a new field (append its number to `docs/spec/format/field-numbers.json`; the guard must pass) giving the lowest schema a reader must implement to apply the record faithfully. A writer sets it to 3 on a record that carries renames and omits it otherwise (absent = the reader floor, 2).
- A reader that implements a lower schema than that field names refuses the record as a typed error and stops replay and catch-up there (never skips it, never applies part of it), and the error says a newer build is needed.
- State the rule generally in FM-9/FM-15 so future semantic fields reuse it, and record in the PR why builds before this change are not protected (they ignore the field; Libraries were recreated at the field-number change and no device needs to run a build from between).
- Rust and TypeScript codecs, interop fixtures, rejection tests.

### 2. The rename operation

- `coffret rename --library <name> --from <from> --to <to>` in the CLI (named, not positional: the pre-parse guard refuses a word after `--passphrase-stdin` as a typed Passphrase) (follow the existing subcommands' argument, `--json` answer, refusal and progress conventions). `<from>` and `<to>` are Entry Paths, validated and normalized as EP-1 does.
- **File.** When `<from>` names a current Entry, rename that Entry to `<to>`.
- **Folder.** When `<from>` names a folder (a prefix of current Entries), rename every Entry under it, each by its own element with the rest of its path kept under `<to>` — the record enumerates them explicitly (rule 8: Entries added concurrently are never swept in).
- **Refusals** (typed, nothing committed, nothing uploaded): `<from>` names nothing; the same path; a destination already held by an Entry not itself renamed away in the same record; a destination that would make a path both a file and a folder (EP's prefix rules); `<to>` under `<from>` for a folder; either side in the reserved `.coffret-derived` namespace (`EntryRename` already refuses it — surface it as the operation's refusal, not an internal error); any other rule FM-15's rename-set check (`check_rename_set`) enforces, checked before anything is written.
- The batch is `PreparedBatch::renaming` prepared against the run's caught-up head, committed through the existing commit path (so a concurrent commit is rebased or answered with `RebaseConflict`, whose CLI message already tells the user to run again).
- After the commit, the run relocates this device's files of the renamed Entries (the same `relocate_waiting` sync and fetch use), so on the device that renamed, the files follow at once; refusals are reported as sync reports them.
- The rename never reads, encrypts or uploads Entry content and never writes a Container; the only Storage writes are the Journal record (and whatever the commit path writes for any record: head, Snapshot when due). Never logs paths beyond their length.
- Expose it from `coffret-device` so the server can add an endpoint later; no server route or explorer UI in this change.

### 3. Spec and concepts

- Library concept: drop "Metadata-only rename is not complete yet … What remains is writing renames"; describe the rename operation in Collocations and Domain Rules (file and folder, explicit enumeration, refusals, no upload, relocation on the renaming device). Keep the sentence that local renames are not inferred (detection is a later task).
- Catalog, Journal, Entry Path, Entry concepts: remove any remaining "nothing writes renames yet"/planned wording.
- EP / CP rules for the operation where they belong (folder rename's enumeration, refusals).

### Tests

- The must-understand rule: a reader implementing schema 2 refuses a record requiring 3 (Rust and TS); a record without renames carries no such field; fixtures round-trip.
- File rename, folder rename (nested, with a Pack holding Entries both inside and outside the folder), swap through two renames in one operation is not offered by the CLI but a rename back works; each refusal leaves Storage untouched.
- Zero re-upload: the store's puts during a rename are only control objects — no Container put (`InMemoryStore::puts_of` or equivalent), for a single file and for a folder spanning several Containers.
- Another device fetches after the rename and finds each file at its new path with no Container re-fetched for content it already holds; the renaming device's files are relocated by the rename run itself.
- A rename racing another device's commit is rebased (follows an update) or refused as `RebaseConflict` per the existing rules, through the CLI path.
- CLI answer tests (text and `--json`) for success and each refusal.

### Out of scope

Local rename detection, the server endpoint and explorer UI, restore/salvage wording.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A Journal record carrying renames names the schema a reader needs; an older reader refuses it with a typed error and stops; Rust and TS codecs and fixtures agree; field-number guard passes.
- [x] `coffret rename` renames a file and a folder (explicit enumeration) in one Journal record and writes no Container (tests count puts).
- [x] Each refusal is typed, reported by the CLI, and commits and uploads nothing (tests).
- [x] After a rename, another device fetches each file at its new path, and the renaming device's files are relocated by the same run (tests).
- [x] The Library, Catalog, Journal, Entry Path and Entry concepts describe rename as implemented (spec checks pass).
