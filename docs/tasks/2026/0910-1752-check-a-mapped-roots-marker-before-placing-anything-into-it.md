---
status: completed
pipeline_phase: null
plan: null
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity, concept-alignment, error-type-design, rust-module-structure]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "pub enum RootRefused" backend/crates/domain/coffret-usecase/src && grep -q "RefusedRoot" backend/crates/apps/coffret-device/src/finding.rs && grep -q "fn a_root_with_no_marker_places_nothing_and_reports_the_mapping" backend/crates/domain/coffret-usecase/tests/place_faults.rs && grep -q "fn a_marker_holding_another_identity_places_nothing_and_reports_the_mapping" backend/crates/domain/coffret-usecase/tests/place_faults.rs && grep -q "fn a_marker_that_is_not_a_regular_file_places_nothing" backend/crates/domain/coffret-usecase/tests/place_faults.rs && grep -q "fn a_marker_over_the_size_cap_places_nothing" backend/crates/domain/coffret-usecase/tests/place_faults.rs && grep -q "fn a_mapping_with_no_expected_identity_places_nothing" backend/crates/domain/coffret-usecase/tests/place_faults.rs && grep -q "fn a_missing_root_still_reports_the_root_and_not_the_marker" backend/crates/domain/coffret-usecase/tests/place_faults.rs && grep -q "fn the_other_mappings_of_the_device_place_as_usual" backend/crates/domain/coffret-usecase/tests/place_faults.rs && grep -q "fn a_root_holding_only_the_management_area_still_reads_as_empty" backend/crates/domain/coffret-usecase/src/local_scan/root_state.rs && grep -rq "fn a_marker_that_is_a_symbolic_link_refuses_placement" backend/crates/gateway/coffret-local-fs/tests'
assignee: null
branch: task/0910-1752-check-a-mapped-roots-marker-before-placing-anything-into-it
created_at: 2026-09-10T17:52:27Z
updated_at: 2026-09-11T03:23:15Z
---

# feat(backend): check a mapped root's marker before placing anything into it

## Overview

Registration now writes a marker at `<mapped root>/.coffret/root` and records
its identifier as the mapping's expected identity (`Mapping::expected_root_id`,
`coffret_usecase::root_marker`, `coffret_usecase::device_state::RootMarkerId`),
but nothing checks it before a write. EP-13 (`docs/spec/entry-path/README.md`)
requires the check before **every placement** — a fetch, an upload the browser
dropped in, a sync write — from the same opened root handle the placement then
uses, and requires the refusal to name the mapping and the reason. This change
lands that check in the local-filesystem capability and the fault-injection
tests that fix it; the reserved-name refusal for `.coffret` paths and the
server / browser reporting follow in the next change.

**Where the check lives.** `Destinations::reach`
(`backend/crates/domain/coffret-usecase/src/destinations.rs`) is the one
capability every placement goes through to descend below a mapped root; it
already opens the root and descends the Entry Path's components without
following links. Give it the expected identity —
`reach(root, expected: Option<&RootMarkerId>, components)` — and have the
gateway (`backend/crates/gateway/coffret-local-fs/src/unix_destinations/`)
verify the marker relative to the **root handle it just opened**, before it
descends the components: open `.coffret` with `O_NOFOLLOW | O_DIRECTORY`,
open `root` with `O_NOFOLLOW` and require a regular file, read at most
`root_marker::MAX_LEN` bytes, `root_marker::parse` them, and compare with
`expected`. Re-resolving the root path for the check would leave exactly the
race EP-13 rules out, which is why the check is inside `reach` and not a
separate probe. `look_up` reads without placing and needs no check. The
in-memory fake (`coffret-usecase/src/in_memory_fs/`) implements the same
contract and gains `plant_marker(root, id)` beside `plant_other`, so the
conformance suite (`destinations_conformance`) states the contract once for
both.

**The refusal.** A new `coffret_usecase::RootRefused` enum (its own module)
names why a root was refused, one variant per EP-13 case:
`NoExpectedIdentity` (the mapping carries none), `ManagementAreaMissing`,
`ManagementAreaNotADirectory`, `MarkerMissing`, `MarkerNotARegularFile`,
`MarkerMalformed { cause: MalformedMarker }`, `MarkerMismatch`. A symbolic link
in either position is the "not the required kind" case, not its own variant.
`reach` returns it through `DescentError` (a new variant carrying
`RootRefused`, beside `Blocked` and `Io`), so the cause stays a value and the
existing `Redacted` discipline applies: the reason and the variant reach a
diagnostic event, the root's path does not (spec: EL-1).

**What a fetch does with it.** A folder fetch continues past a refused mapping
and reports it once: `fetch/run.rs` (or the selection it drives) collects the
first refusal per mapping into a new `Finding::RefusedRoot { local_root,
reason: RootRefused }` in `coffret-device/src/finding.rs`, places nothing under
that mapping, and goes on with the device's other mappings. It is a separate
variant from `Finding::UnavailableRoot` on purpose: EP-12's availability and
EP-13's identity answer different questions, and `RootUnavailable` keeps its
two deliberately asymmetric states. `Display` for the finding names the folder
and says what to do (record the mapping again with `coffret map`, or
`--reset-marker` when the identity is meant to change), in the voice of the
existing `UnavailableRoot` rendering; `Redacted` names only the variant and
the reason.

**What a single writer does with it.** The upload route's
`receive_file` (`coffret-device/src/add/receive_file.rs`) descends through the
same capability; a refusal there fails that request as a whole with a new
`coffret_device::Error` variant carrying `RootRefused` and the root (Display
with the path, Redacted without), the way `Error::descent` already turns a
`Blocked` descent into an error. No new HTTP mapping in this change; the
existing translation of device errors applies.

**EP-12 stays what it was — fix this first.** Since registration writes
`.coffret/` into every mapped root, a root that holds nothing but the
management area is no longer empty to `local_scan/root_state.rs`, whose
availability comparison tests `entries.is_empty()` on `list_folder`; a mapping
recorded while its disk was unmounted therefore reads as *available* afterwards
and EP-12's protection of deletion inference is lost. EP-12's sub-bullet says
the comparison looks past `.coffret/`: make `root_state` ignore entries whose
name `root_marker::is_management_area` accepts when deciding whether the root
holds anything, update its comment, and add
`a_root_holding_only_the_management_area_still_reads_as_empty` to its tests
(an entry named `.coffret` alone → the root reads as empty; `.coffret` plus one
file → it holds files).

**Tests.** In `coffret-usecase/tests/place_faults.rs`, on the in-memory fake
(a marker planted with `plant_marker`; `plant_other` stands in for a symbolic
link or a directory where the file should be):
`a_root_with_no_marker_places_nothing_and_reports_the_mapping`,
`a_marker_holding_another_identity_places_nothing_and_reports_the_mapping`,
`a_marker_that_is_not_a_regular_file_places_nothing`,
`a_marker_over_the_size_cap_places_nothing`,
`a_mapping_with_no_expected_identity_places_nothing`,
`a_missing_root_still_reports_the_root_and_not_the_marker` (EP-12's
`UnavailableRoot`, not `RefusedRoot`),
`the_other_mappings_of_the_device_place_as_usual`. On the real filesystem, one
case in `coffret-local-fs/tests/`:
`a_marker_that_is_a_symbolic_link_refuses_placement`. Every existing fixture
that places into a mapped root now records the mapping with `expecting(id)`
and plants the matching marker (fetch conformance fixtures, `place_faults.rs`,
`fetch_confinement.rs`, `coffret-device`'s `add` / `browse` tests,
`coffret-server/tests/support`); sync and freeze fixtures do not place and
need nothing.

## Acceptance criteria

### Automated (pipeline-verified)

- [ ] The refusal is a typed reason and a fetch reports it per mapping:
      `grep -rq "pub enum RootRefused" backend/crates/domain/coffret-usecase/src`,
      `grep -q "RefusedRoot" backend/crates/apps/coffret-device/src/finding.rs`.
- [ ] The seven fault-injection cases named above exist in
      `coffret-usecase/tests/place_faults.rs` and pass under `make check`.
- [ ] EP-12's emptiness looks past the management area:
      `grep -q "fn a_root_holding_only_the_management_area_still_reads_as_empty" backend/crates/domain/coffret-usecase/src/local_scan/root_state.rs`.
- [ ] A real symbolic link in the marker's place refuses placement:
      `grep -rq "fn a_marker_that_is_a_symbolic_link_refuses_placement" backend/crates/gateway/coffret-local-fs/tests`.
- [ ] Existing backend, frontend, and interoperability checks continue to
      pass, the fetch and destination conformance suites included.

## Out of scope

Refusing an Entry Path that carries a `.coffret` component, refusing a
dropped file into the management area, and reporting either to the browser
(`api_error`, `refusal.ts`) — the next change. Any migration. Automatic
volume discovery. The exFAT and macOS behaviours of `.coffret` (hardware
verification when the integration branch is reviewed).
