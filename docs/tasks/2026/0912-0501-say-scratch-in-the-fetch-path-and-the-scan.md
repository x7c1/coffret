---
status: completed
pipeline_phase: null
follow_up_of: null
base_ref: feat/mapped-root-marker
perspectives: [concept-alignment, completeness, clarity]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && ! grep -rqi "temporary" backend/crates/domain/coffret-usecase/src/scratch.rs && grep -q "The name every scratch written inside a mapped folder begins with" backend/crates/domain/coffret-usecase/src/scratch.rs && ! grep -rqi "temporary" backend/crates/domain/coffret-usecase/src/fetch && grep -q "a fetch could not remove one of its own scratches" backend/crates/domain/coffret-usecase/src/fetch/placement.rs && ! grep -rqi "temporary" backend/crates/domain/coffret-usecase/src/local_scan/walk_mappings.rs && grep -q "a_scratch_a_fetch_left_is_not_a_source_file" backend/crates/domain/coffret-usecase/src/local_scan/walk_mappings.rs && ! grep -rqi "temporary" backend/crates/domain/coffret-usecase/src/destination.rs backend/crates/domain/coffret-usecase/src/flushed_file.rs backend/crates/domain/coffret-usecase/src/local_operation.rs backend/crates/domain/coffret-usecase/src/sync/run.rs backend/crates/domain/coffret-usecase/Cargo.toml'
assignee: null
branch: task/0912-0501-say-scratch-in-the-fetch-path-and-the-scan
created_at: 2026-09-12T05:01:49Z
updated_at: 2026-09-12T13:51:45Z
---

# docs(backend): say scratch in the fetch path and the scan, where the code still says temporary file

## Overview

The register already owns the word. EP-11 defines it:

> A **scratch** is the file a fetch writes before the rename that publishes
> it. It is written inside a mapped folder, which is also a folder a scan
> walks, so coffret reserves a local filename prefix for it: a fetch gives its
> scratches no other kind of name, and a scan passes over every local name
> carrying that prefix instead of reporting it as a file to back up (EP-1,
> EP-8).

The Library concept says the same thing in its vocabulary list — "scratch (a
fetched Entry's bytes to a name under the reserved prefix before the rename
that publishes them)" — and in its Domain Rules: "A fetch writes its
**scratch** — the file it fills before the rename that publishes it — inside a
mapped folder" (spec: EP-11). The plural EP-11 uses is *scratches*.

The documents are consistent; the code is not. `coffret-usecase` has a
`scratch` module, a `scratch::is_scratch` predicate, a `scratch_name` field
and `ScratchFile`/`FlushedFile` ports — and the doc comments around all of
them still call the thing a *temporary file*. `src/fetch/placement.rs` is the
sharpest case: the field is spelled `scratch_name` on line 65 and its own doc
comment one line above says "What the temporary file is called inside it."

### The count

Across `backend/`, the word `temporary` occurs 221 times in 75 files. Sorting
those by what they mean:

| Meaning | Occurrences | Files |
| --- | --- | --- |
| This repository's scratch — drift, must become `scratch` | 100 | 37 |
| The `tempfile` crate's `TempDir`/`tempdir()` ("a temporary directory …") | 91 | 32 |
| An ordinary temporary file or value, not a scratch | 30 | 13 |

The file counts overlap — 82 against 75 distinct files — because a few files
hold both kinds, `coffret-server/tests/` most of all: its fixture helpers
plant throwaway files under a `tempfile` directory *and* assert about a
fetch's scratch.

Outside `backend/`, nothing is drift. `frontend/` contains exactly one match,
`'**/.tmp/**'` in `frontend/eslint.config.js`, which is a lint ignore glob for
tool output; the concept and spec documents already say *scratch*.

This task takes the first 42 of the 100, in the 14 files of `coffret-usecase`
that carry the fetch path itself, the ports it writes through, the scan that
steps over its names, and the module that defines the prefix. The crate's
test-facing code (the conformance suites, `in_memory_fs`, and
`tests/place_faults.rs`) and the gateway and app crates are left to follow-up
tasks; see **Out of scope**.

### The substitutions

Three, applied throughout:

- `temporary file` → `scratch`
- `temporary files` → `scratches`
- `temporary name` / `temporary names` → `scratch name` / `scratch names`

Each replacement below is given in full because these are doc comments whose
line breaks matter: several of them re-wrap when the phrase shortens, and two
of them wrap the phrase itself across two lines.

### What to change

**`backend/crates/domain/coffret-usecase/src/scratch.rs`** (6) — the module
that defines the prefix, so its own prose is the one that has to be right.

- Line 12: `/// The name every temporary file written inside a mapped folder begins with.`
  becomes `/// The name every scratch written inside a mapped folder begins with.`
- Lines 21–22 read `/// So the two flows agree on one prefix: a fetch only ever writes temporary`
  / `/// files whose names begin with it, and a scan passes over every name that does`.
  They become `/// So the two flows agree on one prefix: a fetch only ever writes scratches`
  / `/// whose names begin with it, and a scan passes over every name that does`.
- Line 31: `temporary names` → `scratch names`.
- Line 42: `/// A temporary name nothing else in a destination directory is using.`
  becomes `/// A scratch name nothing else in a destination directory is using.`
- Line 52: `/// A temporary name for a file arriving from outside the Library.`
  becomes `/// A scratch name for a file arriving from outside the Library.`
- Line 57: `/// therefore still write two temporary files, and the second rename is what`
  becomes `/// therefore still write two scratches, and the second rename is what`.

**`src/fetch/placement.rs`** (12)

- Line 30: `The bytes go into a temporary file *in that folder* as they` →
  `The bytes go into a scratch *in that folder* as they`.
- Line 64: `/// What the temporary file is called inside it.` →
  `/// What the scratch is called inside it.` (the field below it is already
  `scratch_name`).
- Line 66: `/// The temporary file until it is flushed, and then what may be published.`
  → `/// The scratch until it is flushed, and then what may be published.`
- Line 90: `/// The folder was reached and a temporary file is open inside it.` →
  `/// The folder was reached and a scratch is open inside it.`
- Lines 110–111 wrap the phrase: `/// Descends to the folder the Entry's file belongs in and opens a temporary`
  / `/// file inside it.` They become
  `/// Descends to the folder the Entry's file belongs in and opens a scratch`
  / `/// inside it.`
- Line 195: `/// Flushes the temporary file to the device and holds it against the` →
  `/// Flushes the scratch to the device and holds it against the`.
- Line 254: `takes the temporary file with` → `takes the scratch with`.
- Line 293: `/// Removes the temporary file, this placement having come to nothing.` →
  `/// Removes the scratch, this placement having come to nothing.`
- Line 322: `/// walk away from the temporary files it had not got to yet.` →
  `/// walk away from the scratches it had not got to yet.`
- Line 342: `/// Removes every temporary file a failed fetch left.` →
  `/// Removes every scratch a failed fetch left.`
- Line 346: the quoted counter-example `"and the temporary file would not go either"`
  becomes `"and the scratch would not go either"`.
- Line 354, the `warn!` message: `"a fetch could not remove one of its own temporary files",`
  becomes `"a fetch could not remove one of its own scratches",`. This is the
  one occurrence an operator reads, so it is the one this task pins.

**`src/fetch/scatter.rs`** (4)

- Line 15: `written to a temporary file where it belongs to a wanted Entry and`
  → `written to a scratch where it belongs to a wanted Entry and`.
- Line 38: `/// Opens a temporary file for every wanted Entry of one Container.`
  → `/// Opens a scratch for every wanted Entry of one Container.`
- Line 148: `/// Closes every temporary file and holds each against the catalog.`
  → `/// Closes every scratch and holds each against the catalog.`
- Line 168: `/// Removes every temporary file, the fetch having come to nothing.`
  → `/// Removes every scratch, the fetch having come to nothing.`

**`src/fetch/mod.rs`** (3)

- Line 73: `//!    temporary file *in that open folder*, are flushed to the device, get the`
  → `//!    scratch *in that open folder*, are flushed to the device, get the`.
- Line 98: `the Keyring, the temporary file and the` → `the Keyring, the scratch and the`.
- Line 114: `//! caller may stop at: a Container read and not placed is temporary files, and a`
  becomes `//! caller may stop at: a Container read and not placed is scratches, and a`.
  The "X is Y" construction is the author's and is kept; only the noun
  changes, and the shorter line leaves lines 115–116 untouched.

**`src/fetch/container.rs`** (3)

- Line 22: `/// temporary file beside where its file will be.` →
  `/// scratch beside where its file will be.`
- Line 68: `writes fresh temporary files — the` → `writes fresh scratches — the`.
- Line 94: `Either way the temporary files this attempt made are gone` →
  `Either way the scratches this attempt made are gone`.

**`src/fetch/decoding.rs`** (2)

- Line 27: `/// temporary file per Entry through.` → `/// scratch per Entry through.`
- Line 140: `/// Removes whatever temporary files this decode had made.` →
  `/// Removes whatever scratches this decode had made.`

**`src/fetch/range_read.rs`** (2)

- Line 75: `writes a fresh temporary file, the` → `writes a fresh scratch, the`.
- Line 137: `Either way the temporary file` → `Either way the scratch`.

**`src/fetch/fetch_request.rs`** (1)

- Line 19: `/// There is no spool directory. A fetch writes its temporary file into the`
  → `/// There is no spool directory. A fetch writes its scratch into the`.

**`src/destination.rs`** (1)

- Line 11: `/// made *relative to it* — the temporary file, the rename that publishes it, the`
  → `/// made *relative to it* — the scratch, the rename that publishes it, the`.

**`src/flushed_file.rs`** (1)

- Line 6: `/// One temporary file whose bytes are on the device, waiting to be given its`
  → `/// One scratch whose bytes are on the device, waiting to be given its`.

**`src/local_operation.rs`** (2)

- Line 47: `/// A spool file, a fetch's temporary file, or a directory above one was`
  → `/// A spool file, a fetch's scratch, or a directory above one was`.
- Lines 61–62 wrap the phrase: `/// A spool file whose Container was committed or abandoned, or a temporary`
  / `/// file a failed fetch left, was being deleted (spec: OC-6, EP-11).`
  They become `/// A spool file whose Container was committed or abandoned, or a scratch`
  / `/// a failed fetch left, was being deleted (spec: OC-6, EP-11).`

**`src/local_scan/walk_mappings.rs`** (3) — the scan side, and the one
identifier this task renames.

- Line 166: `// A temporary file a fetch was killed in the middle of writing. It`
  → `// A scratch a fetch was killed in the middle of writing. It`.
- Line 233: `// EP-11: a fetch writes its temporary file inside the very folder this walk`
  → `// EP-11: a fetch writes its scratch inside the very folder this walk`.
- Line 240: rename the test
  `a_temporary_file_a_fetch_left_is_not_a_source_file` to
  `a_scratch_a_fetch_left_is_not_a_source_file`. The name is declared and used
  only here — it is a `#[cfg(test)]` function in this file's own `tests`
  module — so the rename reaches no other file.

**`src/sync/run.rs`** (1)

- Line 75: `takes over the temporary files a failed` → `takes over the scratches a failed`.

**`Cargo.toml`** (1)

- Line 37, the comment on the `getrandom` dependency:
  `# one Container out of each other's temporary file.` →
  `# one Container out of each other's scratch.`

No behaviour changes, no signatures change, and no identifier outside
`walk_mappings.rs`'s own test module is touched.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `make check` passes.
- [x] `scratch.rs` no longer says *temporary* at all, and states the prefix in
      the register's word:
      `! grep -rqi "temporary" backend/crates/domain/coffret-usecase/src/scratch.rs`
      and
      `grep -q "The name every scratch written inside a mapped folder begins with" backend/crates/domain/coffret-usecase/src/scratch.rs`.
      The strict absence is safe here because this module has no legitimate
      temporary of its own: it names nothing but the reserved prefix.
- [x] The whole `fetch` module says *scratch*, and the operator-facing cleanup
      warning says it too:
      `! grep -rqi "temporary" backend/crates/domain/coffret-usecase/src/fetch`
      and
      `grep -q "a fetch could not remove one of its own scratches" backend/crates/domain/coffret-usecase/src/fetch/placement.rs`.
      The pathspec is the directory `src/fetch` only. It deliberately does not
      reach `src/fetch_conformance/`, which is a follow-up's file set, and it
      cannot reach the `tempfile` crate's "a temporary directory …" messages,
      of which `src/fetch` has none.
- [x] The scan says *scratch* and its test is named for one:
      `! grep -rqi "temporary" backend/crates/domain/coffret-usecase/src/local_scan/walk_mappings.rs`
      and
      `grep -q "a_scratch_a_fetch_left_is_not_a_source_file" backend/crates/domain/coffret-usecase/src/local_scan/walk_mappings.rs`.
      Scoped to this one file rather than `src/local_scan/`, because the rest
      of that module is not in this file set.
- [x] The four remaining files and the manifest comment say *scratch*:
      `! grep -rqi "temporary" backend/crates/domain/coffret-usecase/src/destination.rs backend/crates/domain/coffret-usecase/src/flushed_file.rs backend/crates/domain/coffret-usecase/src/local_operation.rs backend/crates/domain/coffret-usecase/src/sync/run.rs backend/crates/domain/coffret-usecase/Cargo.toml`.
      Named file by file, so that the `tempfile` fixtures elsewhere in the
      crate — `in_memory_fs`, the conformance suites, `tests/` — cannot fail
      it.

No criterion counts occurrences. A gate that pinned a number would turn a
miscount in the survey above into an unfixable check; the absence gates hold
whatever the true count turns out to be.

## Out of scope

These occurrences mean *a temporary file* in the ordinary sense. Renaming any
of them would make the text wrong, so none of them changes:

- **The `tempfile` crate, everywhere in the backend's tests** — 91
  occurrences of "a temporary directory …" in `TempDir::new()` and
  `tempfile::tempdir()` expect messages, plus the module docs that say a
  target "needs nothing but a temporary directory". These name the crate's own
  notion. A scratch is a file a fetch writes inside a mapped folder under a
  reserved prefix; a test's throwaway directory is neither.
- **`backend/crates/apps/coffret-device/src/owner_only.rs`** — the
  `temporary_neighbour` helper, the `temporary` binding it feeds, the `.tmp`
  suffix it builds, and the prose around them ("the point of the temporary
  file is that the rename either happens or does not"). This writes files the
  device keeps for itself, not files inside a mapped folder. Nothing scans
  them, so the reserved prefix does not apply and neither does the word.
- **`backend/crates/gateway/google-drive-store/src/oauth/token_cache/`** —
  the same atomic-replace helper for the OAuth token cache, for the same
  reason.
- **`backend/crates/domain/coffret-format/src/key_envelope.rs`** — "named and
  wiped rather than left as a temporary" describes an unnamed value in memory.
  There is no file in the sentence at all.
- **Fixture writes in test support** —
  `backend/crates/apps/coffret-device/src/run_catch_up.rs` and
  `backend/crates/apps/coffret-server/tests/support/mod.rs` say "a temporary
  file is writable" / "a temporary folder is writable" when planting fixture
  content under a `tempfile` directory. Throwaway fixture files, not
  scratches.
- **`docs/spec/device-key-custody/README.md`, DK-8** — "swap, hibernation
  images, crash dumps, and library temporary files are written outside
  coffret's own writes". Lower-case *library* here, in a list of things the
  operating system and other code write; calling them scratches would claim
  coffret writes them.
- **`frontend/`** — the single match is the `'**/.tmp/**'` ignore glob in
  `frontend/eslint.config.js`, a lint exclusion for tool output. No frontend
  package mentions the scratch area, so `frontend/` has nothing to rename.
- **`scripts/drive-index-layout-it.sh`, `scripts/drive-round-trip-it.sh`** —
  `# temporary:` marks a provisional step in the script, not a file.
- **`docs/tasks/`** — the repository's record of past work. Historical task
  files quote the old wording because it was the wording at the time; they are
  not edited.

Deferred to follow-ups rather than kept, because a single task this wide would
be unreviewable:

- The crate's test-facing code — `src/in_memory_fs/` (3 files),
  `src/destinations_conformance/` and `src/fetch_conformance/` (7 files), and
  `tests/place_faults.rs` — 36 occurrences in 11 files. Note that
  `destinations_conformance/root_arrangement.rs` and
  `destinations_conformance/destinations_under_test.rs` hold `tempfile`
  survivors, so that follow-up's gate must name files rather than sweep the
  directory.
- The gateway and app crates — `coffret-local-fs`'s `src/unix_destinations/`
  (3 files), `coffret-device`'s `src/add/`, `src/entry_fetches/` and
  `src/run_fetch.rs` (5 files), and `coffret-server`'s `src/envelope.rs`,
  `src/fill/run.rs`, `src/routes/upload/receive.rs` and `tests/routes.rs`
  (4 files) — 22 occurrences in 12 files.
