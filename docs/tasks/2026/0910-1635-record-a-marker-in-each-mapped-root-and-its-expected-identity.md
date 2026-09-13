---
status: completed
pipeline_phase: null
plan: null
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -q "SCHEMA_VERSION: i64 = 6" backend/crates/gateway/coffret-sqlite-index/src/schema.rs && grep -q "DEVICE_SCHEMA_VERSION: i64 = 6" backend/crates/gateway/coffret-sqlite-index/src/schema.rs && grep -q "expected_root_id" backend/crates/gateway/coffret-sqlite-index/src/schema.rs && grep -rq "pub struct RootMarkerId" backend/crates/domain/coffret-usecase/src && grep -rq "\"\.coffret\"" backend/crates/domain/coffret-usecase/src && grep -q "reset-marker" backend/crates/apps/coffret-cli/src/map.rs && grep -q "fn recording_a_mapping_writes_a_marker_into_the_root" backend/crates/apps/coffret-device/src/mapping/tests.rs && grep -q "fn recording_a_root_that_already_holds_a_valid_marker_keeps_its_id" backend/crates/apps/coffret-device/src/mapping/tests.rs && grep -q "fn a_root_whose_marker_is_malformed_is_refused_and_nothing_is_written" backend/crates/apps/coffret-device/src/mapping/tests.rs && grep -q "fn a_root_holding_only_the_management_area_is_refused" backend/crates/apps/coffret-device/src/mapping/tests.rs && grep -q "fn a_root_whose_marker_is_a_symbolic_link_is_refused" backend/crates/apps/coffret-device/src/mapping/tests.rs && grep -q "fn resetting_the_marker_issues_a_new_identity" backend/crates/apps/coffret-device/src/mapping/tests.rs && grep -q "fn the_reserved_management_area_is_not_a_source_file" backend/crates/domain/coffret-usecase/src/local_scan/walk_mappings.rs'
assignee: null
branch: task/0910-1635-record-a-marker-in-each-mapped-root-and-its-expected-identity
created_at: 2026-09-10T16:35:15Z
updated_at: 2026-09-10T17:43:00Z
---

# feat(backend): record a marker in each mapped root and its expected identity in device state

## Overview

EP-13 (`docs/spec/entry-path/README.md`) says recording a mapping also
records an identity for the root: a marker file at
`<mapped root>/.coffret/root` holding a random identifier, and the same
identifier kept as the mapping's expected identity in device state. EP-14
reserves the name `.coffret` at any depth under a mapped root. Nothing in the
code does either yet: `coffret map` records a mapping with only a prefix and a
canonicalized local root (`backend/crates/apps/coffret-device/src/mapping/mod.rs`,
`set_mapping`), `Mapping` carries `prefix`, `local_root`, and the
scan-stamped `root_identity` (`backend/crates/domain/coffret-usecase/src/device_state/mapping.rs`),
and the `mappings` table has those three columns
(`backend/crates/gateway/coffret-sqlite-index/src/schema.rs`). This change
lands registration and persistence; the check before placement follows in the
next change on this branch. Read EP-13 and EP-14 first; every rule below is
theirs, restated here only so this file is self-contained.

**The identifier.** Add `RootMarkerId` to `coffret-usecase`'s `device_state`
(its own module, as `root_identity.rs` is): eight random bytes, spelled as
sixteen lowercase hexadecimal characters exactly the way `LibraryId` and
`ContainerId` are spelled (`coffret-model/src/library_id.rs`'s `lowercase_hex`
is the precedent). It needs `generate()` (draw the bytes the way
`coffret-format`'s `generate_library_id` draws them; `coffret-usecase`
depends on `coffret-format`), `parse` from the hex spelling with a typed
error, and `Display` as the hex spelling. No `PartialEq` on any error; the ID
itself compares by value, as `LibraryId` does.

**The marker file.** Add `root_marker.rs` (module name for the marker rule,
in `coffret-usecase`) owning: the reserved directory name `.coffret` and the
file name `root` as constants (so the reserved name lives in one place — put
the `.coffret` constant beside the existing reserved prefix in `scratch.rs`
if that reads better, but only one place either way); the size cap of 64
bytes; `spell(id) -> Vec<u8>` producing the sixteen hex characters followed by
one newline; and `parse(bytes) -> Result<RootMarkerId, MalformedMarker>`
accepting exactly the sixteen characters with at most one trailing newline
and refusing anything else, including input longer than the cap. Unit tests
for spell/parse round-trip, the newline tolerance, and the refusals.

**The mapping and the row.** `Mapping` gains `expected_root_id:
Option<RootMarkerId>`, kept `None` by `Mapping::new` and set by a new
`Mapping::expecting(self, id: RootMarkerId) -> Self` (the same shape as
`stamped`); update the type-level doc, which currently says `root_identity`
"is the one thing a mapping *does* assert" — now a mapping asserts two things
about its root and they answer different questions (EP-12's availability
versus EP-13's identity; cite both). Persist it: `mappings` gains
`expected_root_id TEXT`; bump **both** `SCHEMA_VERSION` and
`DEVICE_SCHEMA_VERSION` to 6 (the table is device-local, so `schema.rs`'s
own rule moves both); `rows::mapping` reads the column through `expecting`
when present, and `rows::refused_mapping` leaves it `None` exactly as it
leaves `root_identity` (a mapping read out of a refused file is about to be
recorded again). Do not widen the two-column promise (`prefix`, `local_root`)
that `RefusedIndex` relies on. With both versions equal, the window
`DEVICE_SCHEMA_VERSION..SCHEMA_VERSION` that `carries_a_readable_device_group`
checks is empty, so the "discard the catalog, keep the device group" path is
unreachable in this build: that is the intended consequence (a device-local
table changed, so nothing older can be kept), and the cases in
`coffret-sqlite-index/tests/schema.rs` that stamp a file with
`DEVICE_SCHEMA_VERSION` to exercise that path must change to state the
consequence — a file stamped 5 is refused as unsupported, the window is empty
— rather than be deleted; keep a unit test of the window predicate itself
with a synthetic pair of versions so the path is still covered. The
hand-written constants in `tests/schema.rs` and `tests/refused_index.rs` move
to 6 with the real ones. No migration: a file stamped 5 is refused, and the
person records their mappings again with `coffret map` (the existing recovery
text in `coffret-cli/src/mappings.rs` already says so). `InMemoryIndex` keeps
whatever `Mapping` it is given.

**Registration.** `coffret_device::set_mapping` writes or adopts the marker
before it records the mapping, from the canonicalized root, opening
`.coffret` and then `root` relative to the root directory handle without
following links (the same no-follow discipline `open_local_file.rs` and the
`coffret-local-fs` gateway use; do it through the existing local-filesystem
capability if one fits, otherwise with `rustix` in `coffret-device` the way
the rest of that crate opens files). The cases, all from EP-13:

- `.coffret` absent: create the directory and the marker with a fresh
  identifier, then record the mapping expecting it. Creation is
  create-exclusive so a concurrent registration cannot overwrite.
- `.coffret` present as a directory and `root` present as a regular file
  whose content parses: **adopt** — record the mapping expecting that
  identifier, write nothing, and tell the person (stderr, see below).
- `root` malformed or over the cap, or `root` or `.coffret` a symbolic link
  or not the required kind, or `.coffret` present without `root`: an error
  naming which of those it was, and nothing written.
- `--reset-marker` (new CLI flag on `coffret map`): where a valid marker
  exists, replace it with a fresh identifier (write the new content to a
  scratch name and rename over `root` so the file is never half-written) and
  record the mapping expecting the new one; where none exists, behave as the
  absent case; where the marker is malformed, still an error — the flag
  replaces an identity, it does not repair a broken area.

The errors are new variants of `coffret_device::Error` whose names say why
(for example `MarkerMalformed`, `MarkerNotARegularFile`,
`ManagementAreaNotADirectory`, `ManagementAreaIncomplete`), each carrying
the root's path for `Display` and rendering through `Redacted` without it
(spec: EL-1). The CLI (`coffret-cli/src/map.rs`) reports on stderr, in the
style of its existing "is at" line, whether it wrote a new marker, adopted the
existing one, or reset it; and the recovery text in
`coffret-cli/src/mappings.rs` need not change unless the wording no longer
holds.

**The scan.** EP-14: a scan never enters a folder named `.coffret` and never
reports anything under it. `local_scan/walk_mappings.rs` already steps over
scratch names through `is_scratch`; add the reserved name beside it (a name
equal to `.coffret` at any depth) with a test
`the_reserved_management_area_is_not_a_source_file` shaped like the existing
`a_temporary_file_a_fetch_left_is_not_a_source_file`. Refusing placement
under `.coffret` and refusing a dropped file into it belong to the next
change, not this one.

**Tests** in `coffret-device/src/mapping/tests.rs` (they use real temporary
directories, so a symbolic link is a real one):
`recording_a_mapping_writes_a_marker_into_the_root`,
`recording_a_root_that_already_holds_a_valid_marker_keeps_its_id` (two
mappings of one root end up expecting the same identifier),
`a_root_whose_marker_is_malformed_is_refused_and_nothing_is_written`,
`a_root_holding_only_the_management_area_is_refused`,
`a_root_whose_marker_is_a_symbolic_link_is_refused`,
`resetting_the_marker_issues_a_new_identity`. Update the existing "old
layout" case to the refusal a file stamped 5 now gets.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] Both schema versions are 6 and the column exists:
      `grep -q "SCHEMA_VERSION: i64 = 6" backend/crates/gateway/coffret-sqlite-index/src/schema.rs`,
      `grep -q "DEVICE_SCHEMA_VERSION: i64 = 6" backend/crates/gateway/coffret-sqlite-index/src/schema.rs`,
      `grep -q "expected_root_id" backend/crates/gateway/coffret-sqlite-index/src/schema.rs`.
- [x] The identifier type and the reserved name exist in `coffret-usecase`:
      `grep -rq "pub struct RootMarkerId" backend/crates/domain/coffret-usecase/src`,
      `grep -rq "\"\.coffret\"" backend/crates/domain/coffret-usecase/src`.
- [x] The CLI offers the explicit new-identity request:
      `grep -q "reset-marker" backend/crates/apps/coffret-cli/src/map.rs`.
- [x] Registration behaves as EP-13 states, shown by the six named tests in
      `coffret-device/src/mapping/tests.rs` listed above, and a scan steps over
      the management area (`the_reserved_management_area_is_not_a_source_file`
      in `local_scan/walk_mappings.rs`), all run by `make check`.
- [x] Existing backend, frontend, and interoperability checks continue to
      pass, with the schema tests restated for the empty window rather than
      removed.

## Out of scope

Checking the marker before a fetch, an upload, or a sync write places
anything, and refusing placement under `.coffret` (the next change). Any
migration of a file stamped 5. Automatic volume discovery or operating-system
volume identifiers. The exFAT and macOS behaviours of `.coffret` (verified on
hardware when the integration branch is reviewed).
