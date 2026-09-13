---
status: completed
pipeline_phase: null
follow_up_of: null
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity, concept-alignment, error-type-design, user-experience]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "ReservedComponent" backend/crates/domain/coffret-usecase/src/fetch/ && grep -rq "carries_management_area" backend/crates/domain/coffret-usecase/src/ && grep -rq "ReservedComponent" backend/crates/apps/coffret-device/src/ && grep -rq "ReservedComponent" frontend/packages/gateway/api/src/ && grep -rq "refused_root" backend/crates/apps/coffret-server/src/ && grep -rq "refused_root" frontend/packages/gateway/api/src/ && grep -rq "reads every finding name the server can send" frontend/packages/gateway/api/src/ && grep -rq "fn a_reserved_component_is_surfaced_and_nothing_is_placed" backend/crates/domain/coffret-usecase/src/fetch_conformance/ && grep -rq "fn a_dropped_file_under_the_management_area_is_refused" backend/crates/apps/coffret-device/src/add/ && grep -rq "fn a_refused_root_reaches_the_browser_as_a_declined_fetch" backend/crates/apps/coffret-server/tests/ && ! grep -q "reserved scratch prefix, which a scan" backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs'
assignee: null
branch: task/0911-1549-refuse-a-reserved-path-component-for-placement-and-report-a-refused-root
created_at: 2026-09-11T15:49:29Z
updated_at: 2026-09-11T16:48:06Z
---

# feat(backend): refuse a reserved path component for placement and tell the browser which mapping was refused

## Overview

Two rules are half-landed and this change finishes both.

**EP-14's placement side is missing.** `docs/spec/entry-path/README.md` (EP-14)
reserves the name `.coffret` as a path component at any depth under a mapped
root and requires two things of it: a scan never enters it and never reports
anything under it as content, and *"a fetch, an upload, and a sync never place a
file at a path carrying that component; an Entry Path carrying it is refused for
placement and reported."* Only the scan side exists.
`coffret_usecase::root_marker::is_management_area`
(`backend/crates/domain/coffret-usecase/src/root_marker.rs:50`) is called from
exactly two places, both scans:
`local_scan/walk_mappings.rs:175` and `local_scan/root_state.rs:95`. Nothing on
the placement side asks the question at all. A fetch whose catalog holds an Entry
at `albums/.coffret/root` translates it through `fetch/translate.rs:327`,
selects it in `fetch/select.rs:54` as an ordinary empty place, and places a file
inside the device's own management area — overwriting the marker EP-13 depends on
in the worst case, and in every case putting a file where the next scan will
never look at it again.

**The reserved-prefix precedent to mirror.** EP-11's `.coffret-fetch-` prefix is
enforced at the *device* layer, on the Entry Path's components, before anything
on disk is reached:
`backend/crates/apps/coffret-device/src/add/receive_file.rs:59` refuses a browser
upload with `FetchError::UnmaterializablePath { path, component: None }`, and
`add/added_at.rs:38` answers `Ok(None)` for the same shape. That is the layer and
the vocabulary to mirror. Note two things about it. Its wire reason is
`unmaterializable`, whose sentence
(`coffret-usecase/src/fetch/fetch_error.rs:320`) already says *"either it names
exactly a mapped root … or it carries coffret's reserved scratch prefix"* — one
refusal spelling out two unrelated causes, which EP-4 (*"the refusal tells
whoever typed it which part of the shape it fails"*) does not really admit, and
which a third cause would make worse. And it has no test: no case anywhere drives
`receive_file`'s reserved-name gate.

**The overlapping-mappings sentence is already satisfied on the scan side** by
the by-name rule (`walk_mappings.rs:175`, case
`the_reserved_management_area_is_not_a_source_file` at
`walk_mappings.rs:282` covers `below/.coffret/root`), and on the placement side
it is satisfied by the same by-name rule once placement asks the question: an
inner root's management area is a `.coffret` component under the outer root, so
refusing the component refuses it. Nothing extra is needed for it.

**The sync writes nowhere today.** `Destinations::reach` is reached only through
`LocalPlace::descend` (`coffret-usecase/src/fetch/local_place.rs:103`), whose two
callers are `fetch/placement.rs:139` and `add/receive_file.rs:69`. The sync flow
places no file into a mapped folder, so EP-14's "a sync never places" needs no
code here; do not go looking for a sync write path.

**EP-13's refusal never reaches the browser.** #117 landed
`RefusedRoot`/`RootRefused` and the seven-variant refusal, but every path out of
it ends in a generic `500`. `coffret-server/src/api_error/from_error.rs:203` puts
`FetchError::RefusedRoot` in the same arm as `Index` and `Io` —
`ApiError::server(cause.redacted())`, whose body says only *"the server could not
answer"* — and `from_error.rs:17`'s catch-all does the same to
`coffret_device::Error::RootRefused`, which is what the browser upload raises.
The fill then stops (`fill/run.rs:153` correctly classifies a refused root as not
about one Entry), so a person whose mapped folder is a copied disk sees
*"could not bring over albums — the server could not answer"* and has no way to
learn that the folder is not the one that was registered. `coffret-server/src/noted.rs:51`
and `:99` already carry the right sentence for a refused root, but nothing
reaches them: `Finding::RefusedRoot` is only produced from a `FetchOutcome`
(`coffret-device/src/findings.rs:100`), and the server builds `Findings` only from
`SyncOutcome` and `FreezeOutcome`. That sentence is dead code on the server today.

**The missing round trip.** The finding names on the wire are
fixed twice: on the backend by
`coffret-server/src/api_error/tests.rs:105`
(`each_finding_travels_by_the_name_the_device_layer_gives_it`, which covers all
five including `UnreachablePlace`), and in the decoder by
`frontend/packages/gateway/api/src/refusal.test.ts` — whose helper is
`refused(status, body)` at `refusal.test.ts:6`, and whose only finding case,
`reads the finding a surfaced refusal stands on` at `refusal.test.ts:32`,
exercises `ForeignFile` alone. `LocallyChanged`, `WitnessedDeletion`,
`UnreachablePlace` and `KeyLost` are in `FINDINGS` (`refusal.ts:201`) with
nothing asserting the decoder accepts them, so a rename or a typo on either side
is caught in neither half. There is no fixture-exchange harness for refusals —
the "round trip" here is that the same literal is asserted on both sides — so the
fix is to make the decoder's case list match the backend's.

### What to build

**1. One predicate, asked on the placement side.** Add to
`coffret-usecase/src/root_marker.rs`, beside `is_management_area`:

```rust
/// Whether an Entry Path carries the reserved name at any depth (spec: EP-14).
pub fn carries_management_area(path: &EntryPath) -> bool
```

It splits on `/` and asks `is_management_area` of each component, exactly as the
scratch gate at `receive_file.rs:59` does. It is name-only and does no I/O, which
is what keeps it in this module.

**2. A finding for the fetch.** Add `Surfaced::ReservedComponent { path:
EntryPath }` to `coffret-usecase/src/fetch/surfaced.rs`, with the `path()` arm.
Ask the predicate at the top of the per-target loop in
`coffret-usecase/src/fetch/select.rs:65`, before `place.look(...)`: a path
carrying the reserved name is pushed as that finding and the loop goes on. That
single site gives both propagations for free, because `fetch_entry`
(`fetch/entry_run.rs:77`) runs the same `select` over a one-target vector: a
folder fetch reports the Entry and places the rest, and a single-Entry fetch
returns `EntryFetch::Surfaced(...)`, which the file route already answers as
`409 declined`. Do **not** put the check in `translate` — `targets()` propagates
its error with `?`, so a refusal there would fail the whole folder fetch instead
of reporting one Entry.

**3. A refusal for the upload, and the EP-4 cleanup it forces.** Add
`FetchError::ReservedComponent { path: EntryPath, component: String }` to
`coffret-usecase/src/fetch/fetch_error.rs`, whose message names the component
that made it so, and whose `Redacted` rendering is
`Fetch::ReservedComponent(path_len=N)` — neither the path nor the component
reaches a diagnostic event (spec: EL-1). Move the existing scratch-prefix refusal
at `add/receive_file.rs:59` onto it and extend the same gate to the management
area, so one function decides both reservations:

```rust
let reserved = path.as_str().split('/').find(|component| {
    scratch::is_scratch(component) || root_marker::is_management_area(component)
});
```

Extend `add/added_at.rs:38`'s gate the same way — a path under the management
area is never a local file of this device's, for the reason a scratch name is not
one. With the scratch case moved out, delete the "either … or …" clause from
`UnmaterializablePath { component: None }`'s message
(`fetch_error.rs:320`): it then has exactly one cause left — a path naming
exactly a mapped root — and says that and nothing else.

**4. The reporting chain, one variant at a time.** `Surfaced::ReservedComponent`
forces, in order: `FindingReason::ReservedComponent`
(`coffret-device/src/finding_reason.rs`, with a `Display` sentence saying the name
is coffret's own folder and never content), the `declined` arm in
`coffret-device/src/findings.rs:135`, the `ApiError::declined` arm and `name_of`
in `coffret-server/src/api_error/mod.rs:172` and `:424` (reason `surfaced`,
finding name `ReservedComponent`; update the documented finding set at
`api_error/mod.rs:76`), the `said` arm in `coffret-server/src/noted.rs:130`, and
`SurfacedFinding` plus `FINDINGS` in
`frontend/packages/gateway/api/src/refusal.ts:67` and `:201`.
`FetchError::ReservedComponent` forces the `from_fetch` arm in
`api_error/from_error.rs:164` — `declined_as("reserved", …)`, a new
`DeclinedReason` beside `unmapped` and `unmaterializable`, mirrored into
`REASONS` and the `DeclinedReason` union in `refusal.ts:59`/`:193` — and an arm
in `fill/run.rs:129`'s `is_about_one_entry`, where it is `true`: a reserved path
says nothing about the next file.

**5. The refused root reaches the browser.** In
`api_error/from_error.rs`, take `FetchError::RefusedRoot` out of the `500` arm and
give it `ApiError::refused_root(&RootRefused)`: `409`, kind `declined`, a new
reason `refused_root`, no finding, `cause` set to the redacted rendering for the
log. Add the matching arm for `coffret_device::Error::RootRefused` in the
`impl From<Error> for ApiError` match at `from_error.rs:8`, so a browser upload
into a copied disk is answered the same way a fetch is. The sentence is the one
`noted.rs:99` already writes — *"a folder this device maps is not the folder it
was set up against, so nothing was put into it; record the mapping again with
`coffret map`"* — so write it once and have `noted::refused` use the same text
rather than keeping two copies. **The local root does not go in the body**, for
the reason `noted.rs`'s own header gives: a local path does not cross this
boundary. Which mapping it was reaches the person through the folder the line
already names. Add `'refused_root'` to `DeclinedReason` and `REASONS` in
`refusal.ts`.

**6. The UI needs no new code, and the change has to prove that.** With the
refusal carrying a real sentence, `fillLine`
(`frontend/packages/apps/web/src/fill.ts:105`) already renders
*"could not bring over albums — a folder this device maps is not the folder it
was set up against, so nothing was put into it; record the mapping again with
`coffret map`"*, and `rowFill` (`fill.ts:54`) already renders a declined row's
message. Add a case to `frontend/packages/apps/web/src/fill.test.ts` fixing that
— a stopped fill whose `stopped` is the refused-root refusal produces a line
carrying the sentence — instead of changing the explorer. Do not redesign the
status bar.

### Tests

- `coffret-usecase/src/fetch_conformance/conflicts.rs`:
  `a_reserved_component_is_surfaced_and_nothing_is_placed`, registered in
  `fetch_conformance/mod.rs`'s `pub use` list and in the
  `fetch_conformance!` case list (`mod.rs:118`). Use
  `fetch_conformance::fixtures::plant` — which exists precisely for "a Library
  state no sync produces" — to commit an Entry at `albums/.coffret/root`
  alongside an ordinary `albums/spring.jpg`, then `fetch_folders`: the ordinary
  Entry is placed, the reserved one is a `Surfaced::ReservedComponent`, the run
  succeeds, and the target folder's `.coffret/root` still holds the marker the
  fixture planted. A scan can never produce such an Entry, and that is the point
  — EP-14 defends against a path another device committed.
- `coffret-device/src/add/tests.rs`:
  `a_dropped_file_under_the_management_area_is_refused` and
  `a_dropped_file_under_the_scratch_prefix_is_refused` — both through
  `drop_file`/`refused_path` (`add/tests.rs:109`, `:120`), asserting the refusal
  names the component and that nothing was written. The second closes the gap
  that `receive_file.rs:59` has no test at all today.
- `coffret-server/src/api_error/tests.rs`: `ReservedComponent` added to
  `each_finding_travels_by_the_name_the_device_layer_gives_it`; a new case
  asserting every one of the seven `RootRefused` variants comes back as
  `(409, "declined", Some("refused_root"), None)`; and a case in the recording
  block asserting a refused root writes `Device::RootRefused: <variant>` and no
  path.
- `coffret-server/tests/routes.rs`:
  `a_refused_root_reaches_the_browser_as_a_declined_fetch` — take the served
  device's mapped root (`register_root` in `tests/support/mod.rs:629` planted the
  marker), overwrite `.coffret/root` with another identity, then
  `GET /api/file?path=albums/notes.txt` and assert `409`,
  `error == "declined"`, `reason == "refused_root"`, and a `message` naming
  `coffret map`. Assert the same refusal appears as the fill's `stopped` in
  `GET /api/activity`. This is the only place the real JSON is produced, so it is
  the backend half of the round trip.
- `frontend/packages/gateway/api/src/refusal.test.ts`: a case
  `reads every finding name the server can send` iterating all six
  `SurfacedFinding` literals — `ForeignFile`, `LocallyChanged`,
  `WitnessedDeletion`, `UnreachablePlace`, `KeyLost`, `ReservedComponent` —
  through the existing `refused()` helper and asserting the decoder returns each
  rather than `null` (this closes the `UnreachablePlace` gap), and a case
  `reads a refused mapped root as its own declined reason` for
  `reason: 'refused_root'`. Keep the existing
  `drops a reason and a finding it has never heard of` case as it is.
- `frontend/packages/apps/web/src/fill.test.ts`: the stopped-line case above.

### Concept document

Add one bullet to `docs/concepts/entry-path/README.md`, under the fetch/place
rule near line 82: the name reserved for the device's own area is refused for
placement at any depth and the refusal is reported, with the same
no-silent-selection reasoning the declining bullet beside it carries
(spec: EP-14, EP-4). One bullet, in the register the file already uses.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] An Entry Path carrying `.coffret` is refused for placement by a name-only
      predicate rather than by a descent:
      `grep -rq "carries_management_area" backend/crates/domain/coffret-usecase/src/`
      and `grep -rq "ReservedComponent" backend/crates/domain/coffret-usecase/src/fetch/`.
- [x] The refusal travels the chain to the browser by name:
      `grep -rq "ReservedComponent" backend/crates/apps/coffret-device/src/` and
      `grep -rq "ReservedComponent" frontend/packages/gateway/api/src/`.
- [x] A folder fetch reports such an Entry and places the rest, proven by
      `a_reserved_component_is_surfaced_and_nothing_is_placed` in the fetch
      conformance suite:
      `grep -rq "fn a_reserved_component_is_surfaced_and_nothing_is_placed" backend/crates/domain/coffret-usecase/src/fetch_conformance/`.
- [x] A browser upload into the management area is refused before anything is
      written:
      `grep -rq "fn a_dropped_file_under_the_management_area_is_refused" backend/crates/apps/coffret-device/src/add/`.
- [x] A refused mapped root reaches the browser as its own declined reason rather
      than as a `500`:
      `grep -rq "refused_root" backend/crates/apps/coffret-server/src/` and
      `grep -rq "refused_root" frontend/packages/gateway/api/src/`.
- [x] A real request against a root whose marker names another identity answers
      `409 declined` / `refused_root`:
      `grep -rq "fn a_refused_root_reaches_the_browser_as_a_declined_fetch" backend/crates/apps/coffret-server/tests/`.
- [x] Every finding name the server can send is decoded by the TypeScript client,
      `UnreachablePlace` included:
      `grep -rq "reads every finding name the server can send" frontend/packages/gateway/api/src/`.
- [x] `UnmaterializablePath { component: None }`'s message names one cause rather
      than two, the scratch-prefix refusal having moved to `ReservedComponent`:
      `! grep -q "reserved scratch prefix, which a scan" backend/crates/domain/coffret-usecase/src/fetch/fetch_error.rs`.
- [x] Existing backend, frontend and interoperability checks continue to pass —
      the fetch, destination, sync and freeze conformance suites, the router
      tests, and `make interop` included.

## Out of scope

- Renaming `a_root_with_no_marker_refuses_the_reach` /
  `a_root_with_no_marker_places_nothing_and_reports_the_mapping`.
- Concept-document additions naming the EP-13 verdict a "refused root" and
  documenting the two shapes of *vouch* — a separate documentation change. The
  one EP-14 placement bullet above is this change's; nothing else in
  `docs/concepts/` moves.
- The `Errno::MLINK` gap in the registration layer, the `"temporary file"` →
  scratch rename, and `DescentError`'s `path` / `component` vocabulary crossing.
- Making a fetch refuse an Entry Path that carries the `.coffret-fetch-` scratch
  prefix. EP-11 does not require it and today's fetch does not do it; only the
  refusal's *spelling* moves here, not its reach. Record it as a follow-up if the
  reviewer wants the fetch and the upload to agree on that one too.
- Refusing `.coffret` as a mapping's top-level prefix at registration time
  (`coffret map`). A mapping keyed on that component would leave no reserved
  component below the root for this check to see; it is a registration-layer
  question and belongs with the other registration work.
- Any migration or compatibility read. Backward compatibility is not a goal of
  this project.
- Reading a file the Library holds under a `.coffret` path. EP-14 governs
  placement; the explorer's read routes are unchanged.
