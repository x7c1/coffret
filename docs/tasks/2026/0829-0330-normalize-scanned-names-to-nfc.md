---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && (cd backend && RUSTDOCFLAGS=-Dwarnings cargo doc --workspace --no-deps --document-private-items) && grep -q 'unicode-normalization' backend/crates/domain/coffret-usecase/Cargo.toml && git grep -q 'nfc' -- backend/crates/domain/coffret-usecase/src/local_scan && ! grep -q 'Canonicalization is not implemented yet' backend/crates/domain/coffret-model/src/entry_path.rs && git grep -qF 'an_nfd_local_name_becomes_an_nfc_entry_path' -- backend/crates/domain/coffret-usecase/src/sync_conformance"
assignee: null
branch: task/0829-0330-normalize-scanned-names-to-nfc
created_at: 2026-08-29T03:30:00Z
updated_at: 2026-08-29T04:35:00Z
---

# fix(backend): normalize scanned local names to NFC before they become Entry Paths

## Overview

EP-1 (`docs/spec/entry-path/README.md:13`) requires every Entry Path component
to be valid Unicode **normalized to NFC**, and the Index conformance suite
already states the intended mechanism — "NFC is the canonical form an Entry
Path is put into before it ever reaches the catalog"
(`index_conformance/paths.rs:64`) — but no code performs that normalization:
`EntryPath` carries whatever string it is given
(`coffret-model/src/entry_path.rs:5`–`:8`, "Canonicalization is not
implemented yet"), and the scan builds paths verbatim from file names
(`local_scan/walk_mappings.rs:116`–`:117`, `:162`). On macOS — a 1st-release
target — the filesystem hands names back in NFD, so the same logical file
scanned on macOS and on Linux would today produce two different Entry Paths,
and a fetch that materialized an NFC name could see the very file it wrote
reported as a new file on the next scan. This task implements EP-1's
normalization at the one boundary where operating-system text becomes an
Entry Path.

**Where to normalize — the local-scan boundary, not the type.** `EntryPath`
values also come from decoding stored payloads (Journal records, Snapshots,
Container metadata), which EP-1 already requires to be NFC on the wire;
normalizing inside `EntryPath::new` would silently rewrite whatever a stored
object carries and break byte-exact round-trips (encode/decode equality, the
interop fixtures, digests over encoded forms). Stored data stays verbatim —
a non-NFC path in a stored object is a validation question for the format
layer and is out of scope here. What must normalize is the untrusted OS text:

1. **The scan** (`local_scan/walk_mappings.rs` — shared by sync and freeze):
   every relative-path component read from the filesystem is NFC-normalized
   before `entry_path()` assembles the Entry Path. Use the
   `unicode-normalization` crate (add it to `coffret-usecase`'s
   dependencies) — Unicode normalization tables are not something to
   hand-roll, which is exactly the bar the house rule sets for a third-party
   dependency. Normalize per component (the separator `/` is ASCII and
   unaffected; normalizing the joined string is equivalent, but per-component
   keeps the operation next to the `to_str` boundary that already rejects
   non-Unicode names).
2. **Any other usecase-layer site where OS- or user-supplied text becomes an
   `EntryPath`** — survey `EntryPath::new` callers in `coffret-usecase`: a
   mapping prefix taken from configuration or request input normalizes the
   same way (a mapping keyed by an NFD prefix would never match NFC-scanned
   paths, EP-9); paths decoded from stored payloads and paths built from
   already-normalized Entry Paths do not. State the rule where the
   normalization lives: *text from outside the Library normalizes on the way
   in; text the Library already holds is already NFC*.
3. **The model doc** (`coffret-model/src/entry_path.rs:5`–`:8`): the
   "Canonicalization is not implemented yet" paragraph is now false — reword
   it to say the type carries the path verbatim and that normalization is the
   constructing boundary's job, citing EP-1 as plain text (the phrase
   `Canonicalization is not implemented yet` must be gone; the check command
   gates on its absence).

**No migration.** Existing uploaded data predating this change is test data
and is not migrated or specially handled; the Library's stored paths are
treated as NFC per EP-1.

**Tests.** House-style conformance cases (exported + registered in the suite
macro so they run against every backend):

- `sync_conformance::an_nfd_local_name_becomes_an_nfc_entry_path` — plant a
  local file whose name is NFD on disk (e.g. `e` + combining acute; create it
  via the byte string so the test is filesystem-independent), sync, and
  assert the committed Entry Path is the NFC spelling. Then sync again and
  assert nothing is reported as modified or new — the round-trip is stable on
  a filesystem that preserved the NFD name.
- Extend or add a freeze-side assertion only if the freeze path does not
  already go through the same normalized scan (it should — `local_scan` is
  shared; verify rather than duplicate).
- Keep the existing
  `index_conformance::normalization_form_distinguishes_two_entry_paths`
  untouched — the catalog still treats different byte forms as different
  paths (EP-3); it is the boundary above it that now guarantees only NFC
  arrives.

Conventions per `CLAUDE.md`: `make check` as the gate, English throughout,
Conventional Commits, self-contained commit and PR text, no `PartialEq` on
error types (none should be touched here). `coffret-logging` rules hold —
Entry Paths never appear in log fields, so no logging changes.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] The scan NFC-normalizes every name it reads from the filesystem before
      it becomes part of an Entry Path (check gates: `unicode-normalization`
      in `coffret-usecase`'s Cargo.toml, `nfc` used under `local_scan/`), and
      `sync_conformance::an_nfd_local_name_becomes_an_nfc_entry_path` passes
      (check gate on the case name; the suite macro runs it against in-memory
      and MinIO backends) — including the second-sync half asserting a
      normalized Entry is not re-reported.
- [x] Every usecase-layer site where OS- or user-supplied text becomes an
      `EntryPath` normalizes; sites that decode stored payloads or derive
      from existing Entry Paths are untouched, so encode/decode round-trips
      and interop fixtures are byte-identical (`make check`'s interop job and
      the format test suites pass unmodified).
- [x] The `EntryPath` model doc no longer claims canonicalization is
      unimplemented (check gate: the phrase is absent) and instead states
      where normalization happens, citing EP-1 as plain text.
- [x] `make check` and `RUSTDOCFLAGS=-Dwarnings cargo doc --workspace
      --no-deps --document-private-items` are clean, and
      `index_conformance::normalization_form_distinguishes_two_entry_paths`
      still passes unchanged in meaning.

## Out of scope

- **Format-layer validation of stored paths** (rejecting a stored object
  whose Entry Path is not NFC) — a separate decision about handling
  malformed stored data.
- **Migration of existing uploaded data** — decided unnecessary; current
  Drive-side data is test data.
- **Case folding, width folding, or any normalization beyond NFC** — EP-3
  deliberately keeps equality exact.
- **`docs/spec/` and `docs/concepts/` changes** — EP-1 already states the
  rule; nothing new to register.
