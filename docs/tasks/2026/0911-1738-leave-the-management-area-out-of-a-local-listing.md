---
status: completed
pipeline_phase: null
follow_up_of: null
base_ref: feat/mapped-root-marker
perspectives: [completeness, clarity, concept-alignment, error-type-design]
max_refine_rounds: 3
retries_remaining: 1
check_command: 'make check && grep -rq "fn nothing_under_the_management_area_is_listed_as_a_local_file" backend/crates/apps/coffret-device/src/add/ && grep -rq "Errno::MLINK" backend/crates/apps/coffret-device/src/mapping/ && grep -rq "getrandom::Error" backend/crates/domain/coffret-format/src/ && grep -rq "fn an_entropy_failure_carries_what_the_source_reported" backend/crates/domain/coffret-format/src/ && grep -rq "fn a_root_with_no_management_area_refuses_the_reach" backend/crates/domain/coffret-usecase/src/destinations_conformance/ && grep -rq "fn a_root_with_no_management_area_places_nothing_and_reports_the_mapping" backend/crates/domain/coffret-usecase/tests/ && ! grep -rq "a_root_with_no_marker" backend/crates/domain/coffret-usecase/ && ! grep -rq "the mapping cannot vouch for" backend/crates/domain/coffret-usecase/src/ && ! grep -rq "one marker locks every Entry" backend/crates/ && grep -rq "key-lost marker" backend/crates/apps/coffret-device/src/ && grep -rq "EP-13" backend/crates/domain/coffret-usecase/src/fetch/entry_run.rs && grep -rq "EP-14" backend/crates/apps/coffret-server/src/routes/upload/receive.rs'
assignee: null
branch: task/0911-1738-leave-the-management-area-out-of-a-local-listing
created_at: 2026-09-11T17:44:17Z
updated_at: 2026-09-11T18:28:45Z
---

# fix(backend): leave the management area out of a local listing, and align the vocabulary around it

## Overview

Two rules landed recently: a mapped root carries an identity of its own in a
marker file at `<root>/.coffret/root` (spec: EP-13), and `.coffret` is reserved
as a path component at any depth under a mapped root (spec: EP-14). Placement,
registration and the browser-facing refusals were brought to both. What was left
behind are the places that read the same names, or the same word, and were not
visited: one listing that still reports the device's own marker as somebody's
file, one errno reading that is narrower on the registration side than on the
placement side, one error that flattens its cause into a string, and a handful of
sentences and test names that now say the wrong thing because *marker* and
*vouch* mean something they did not mean before.

Nothing here changes a protocol, an on-disk format, or a wire shape. Three of the
changes are behavioural and carry tests; the rest are the vocabulary the next
reader of these files will be reading.

### 1. A local listing reports the device's own marker as a file to back up

`OpenLibrary::added_locally`
(`backend/crates/apps/coffret-device/src/add/added_locally.rs`) answers "the
files in one folder of the Library that this device has and the Library does
not". It reads the folder directly through `MappedRoots::list_folder`
(`added_locally.rs:63`) rather than through a scan, and the only reserved name it
steps over is the scratch prefix (`added_locally.rs:82`). The management area is
not among them.

So `GET /api/list?path=.coffret` lists the marker. `local_folder_for`
(`coffret-usecase/src/fetch/translate.rs:132`) resolves `.coffret` through the
Library-root mapping like any other folder — `reaching` falls back to the root
mapping for a component nothing claims (`translate.rs:182`) — the directory read
returns `root`, the catalog holds no Entry for it, and `child_path` gives it
`.coffret/root`. The row reaches the browser through `routes/list.rs:96`, merged
in among the Library's own rows by `merged` (`list.rs:120`).

That is against EP-14's scan clause — *"it never enters a folder of that name and
never reports anything under it as a file to back up"* — and a listed local file
is exactly a file to back up: it is the row that appears the moment something is
dropped into a mapped folder, waiting for the sync that will commit it. The scan
already keeps the rule (`local_scan/walk_mappings.rs:174`), and so does the
single-path form of this very question: `added_at`
(`add/added_at.rs:40`) answers `Ok(None)` for any path carrying the reserved
component. The two disagree today, which produces a row that is listed and
cannot be opened.

**The change.** In `added_locally`:

- Ask the reservation of the folder being read, before the directory read: a
  folder whose Entry Path carries the reserved component has no files of this
  device's in it, so the answer is an empty vector. Use
  `coffret_usecase::root_marker::carries_management_area`, which
  `Option::is_some_and` takes directly. An empty answer rather than a refusal, for
  the reason the doc comment already gives for a folder no mapping reaches
  (spec: EP-9).
- Ask it of each name in the directory read, beside `scratch::is_scratch` at
  `added_locally.rs:82`, so a name reserved at any depth is stepped over whatever
  stands at it. A folder of that name is passed over today only because the kind
  check below drops folders; an ordinary *file* called `.coffret` inside a mapped
  folder is reported. Mirror `walk_mappings.rs:174`, which decides the same
  question from the name alone.
- Extend the "Two names are left out" paragraph in the doc comment to three, in
  the voice it is written in: the third is the device's own area, reserved by
  name at any depth (spec: EP-14), and saying so is what keeps this answer and
  `added_at`'s from disagreeing about what a local file of this device's is.

**The test.** `backend/crates/apps/coffret-device/src/add/tests.rs`,
`nothing_under_the_management_area_is_listed_as_a_local_file`. The `device()`
fixture (`add/tests.rs:59`) already registers its root, so `<root>/.coffret/root`
is on disk with a marker in it — the exact state the defect needs. Assert three
things: `added_locally(Some(".coffret"))` is empty (today it reports
`.coffret/root`); a regular file written at `<root>/albums/.coffret` beside an
ordinary `<root>/albums/spring.jpg` leaves `added_locally(Some("albums"))`
reporting the ordinary file alone; and the marker is still on disk afterwards,
since reading a folder never writes to it.

### 2. The registration layer reads one of the two errnos for a symbolic link

A symbolic link at either reserved name is refused rather than followed, and the
placement side reads both spellings of the refusal. `unix_destinations/vouch.rs`
matches `Errno::LOOP | Errno::MLINK | Errno::NOTDIR` for the management area
(`vouch.rs:56`) and `Errno::LOOP | Errno::MLINK` for the marker (`vouch.rs:81`),
with the comment that says why: `O_NOFOLLOW` reports `ELOOP`, *"`EMLINK` where
the BSDs spell it that way"*. The descent's own reading agrees
(`unix_destinations/mod.rs:146`).

The registration layer matches `Errno::LOOP` alone, in three arms:

- `backend/crates/apps/coffret-device/src/mapping/root_marker/read_marker.rs:38`
  — a link at the marker's name, which should be `Error::MarkerNotARegularFile`.
- `backend/crates/apps/coffret-device/src/mapping/root_marker/management_area.rs:57`
  — a link at `.coffret`, which should be `Error::ManagementAreaNotADirectory`.
- `management_area.rs:80` — the same question asked again after a racing
  `mkdirat` answered `EEXIST`.

On a BSD-family system all three fall through to the catch-all and become
`Error::local(...)`: recording a mapping over a root whose `.coffret` is a
symbolic link reports a local I/O failure at a path instead of the typed verdict
EP-13 names, and the person is not told that a reserved name has something else
standing at it.

**The change.** Add `Errno::MLINK` to each of the three arms, and say why in the
comment at each site, mirroring `vouch.rs:50` rather than inventing a second
explanation. `management_area.rs:48` already explains the `ELOOP`/`ENOTDIR`
pairing for the first of the three; extend that paragraph rather than adding a
new one.

**The test.** `a_root_whose_marker_is_a_symbolic_link_is_refused`
(`backend/crates/apps/coffret-device/src/mapping/tests.rs:391`) already drives
both names and asserts both typed verdicts, and it stays the test that holds this
true. Be explicit about what it can and cannot prove: on Linux a link at either
name reports `ELOOP`, so no test in this suite drives the `EMLINK` arm, exactly
as none drives the gateway's. That is why the arm carries a comment saying which
platform spells it that way — the same construction the gateway relies on — and
why confirming the arm on such a host is left out of scope here rather than
claimed as an automated criterion.

### 3. `coffret-format` flattens a `getrandom::Error` into a string

`coffret_format::Error::EntropyUnavailable { detail: String }`
(`backend/crates/domain/coffret-format/src/error.rs:647`) is built at the one
place this crate draws randomness: `entropy.rs:12` calls `error.to_string()` and
keeps the sentence. The value is gone, so nothing downstream can read the
source's kind, and `error::Error::source` (`error.rs:972`) answers `None` for it —
the cause chain stops at a rendered message.

The workspace already has the answer to this, in the other crate that draws
entropy: `google-drive-store`'s `Error::EntropyUnavailable { cause:
getrandom::Error }` (`gateway/google-drive-store/src/error.rs:142`) carries the
value, renders it in its message (`:363`), returns it from `source()` (`:388`),
and is tested with `getrandom::Error::UNSUPPORTED` (`:699`). `getrandom::Error`
is `Copy + Clone` and implements `core::error::Error`, so it fits
`coffret_format::Error`'s existing `#[derive(Debug, Clone)]` unchanged.

**The change.**

- `coffret-format/src/error.rs`: `EntropyUnavailable { cause: getrandom::Error }`,
  the field documented as what the entropy source reported; the `Display` arm at
  `error.rs:963` renders the cause; and a `source()` arm returning `Some(cause)`.
  `source()` is currently a two-arm match with a `_ => None` catch-all — add the
  arm above it rather than restructuring the match.
- The `Redacted` doc comment (`error.rs:981`) claims every variant but `Model` is
  safe to write down as it stands, on the grounds that each describes the bytes
  of an object. A foreign cause is now rendered through that blanket arm, so say
  why it holds for this one: `getrandom` renders a fixed sentence about this
  machine's random source and names no path, no filename and no content
  (spec: EL-3, EL-4). One sentence in the paragraph that already carries the
  `Model` exception.
- `coffret-format/src/entropy.rs:12`: `.map_err(|cause| Error::EntropyUnavailable
  { cause })`.
- `backend/crates/apps/coffret-device/src/error.rs:1242` constructs one in the
  refusal-rendering case. Build it with `getrandom::Error::UNSUPPORTED`, and
  compose its expected rendering with `format!` from that constant's own `Display`
  rather than hard-coding the sentence `getrandom` happens to print — a reworded
  upstream sentence must not break a case about this layer's rendering. The case
  compares `error.redacted()` exactly, so the tuple's expected value becomes a
  `String`.

**The test.** `coffret-format/src/error.rs`'s `mod tests`,
`an_entropy_failure_carries_what_the_source_reported`: the refusal's `source()` is
`Some`, its `Display` carries the source's own sentence, and its `redacted()` is
`Format: could not draw random bytes: <the source's sentence>` — composed from
`getrandom::Error::UNSUPPORTED` rather than written out, for the reason above.

### 4. `vouching.rs` makes the mapping the subject of *vouch*

This repository has two shapes of *vouch* and the mapping is neither of them.
EP-12 is the **device** vouching for a mapped root — *"A mapped root this device
cannot vouch for"* (`docs/concepts/library/README.md:89`), *"a mapped root the
device cannot vouch for"* (`docs/spec/pack-construction/README.md:96`). EP-13 is
the **root** vouching for itself, which is how the rest of this branch says it:
`fetch/placement.rs:92`, `fetch/run.rs:44`, `fetch/scatter.rs:32`,
`routes/upload/mod.rs:111`.

Two strings in
`backend/crates/domain/coffret-usecase/src/destinations_conformance/vouching.rs`
say *a root the mapping cannot vouch for* instead, taking the mapping as the
subject: the `.expect(...)` at `vouching.rs:28` and the assertion message at
`vouching.rs:104`. Both are about the EP-13 check, so both take the established
phrasing — a root that will not vouch for itself.

### 5. Two conformance cases are named after the wrong verdict

`a_root_with_no_marker_refuses_the_reach` (`vouching.rs:44`) does not exercise a
root with no marker. It arranges a root with no management area at all and
asserts `RootRefused::ManagementAreaMissing` (`vouching.rs:49`) — and it sits
directly beside `a_management_area_with_no_marker_refuses_the_reach`
(`vouching.rs:79`), which is the case that really is about a missing marker and
asserts `RootRefused::MarkerMissing`. Two adjacent cases whose names do not tell
a reader which is which, in a suite whose whole point is that the two verdicts
are told apart. `place_faults.rs:321`'s
`a_root_with_no_marker_places_nothing_and_reports_the_mapping` has the same
misnomer for the same arrangement.

Rename both after what they arrange:

- `vouching.rs:44` → `a_root_with_no_management_area_refuses_the_reach`, and with
  it the `pub use` list at
  `destinations_conformance/mod.rs:111` and the `destinations_conformance!` case
  list at `mod.rs:141`.
- `place_faults.rs:321` →
  `a_root_with_no_management_area_places_nothing_and_reports_the_mapping`.

Earlier changes on this branch put this rename out of scope because their own
acceptance gates cited the old names. No gate constrains them now: the only greps
naming them live in the `check_command` of a task file that is already
`status: completed`, and a completed task's check is never re-run. Leave those
historical task files exactly as they are.

### 6. Bare *marker* now collides with the mapped root's marker

Three doc comments say *"one marker locks every Entry the Container holds"* about
the Keyring's key-lost marker (spec: KL-7, KL-17):

- `backend/crates/domain/coffret-usecase/src/fetch/fetch_outcome.rs:51`
- `backend/crates/apps/coffret-device/src/finding.rs:71`
- `backend/crates/apps/coffret-device/src/findings.rs:259`

Each of them now sits a few lines from a field about the *other* marker.
`fetch_outcome.rs:51` is six lines below `refused: Vec<RefusedRoot>`, whose own
comment is about the root's recorded identity; `finding.rs:71` is four lines
below `RefusedRoot`'s *"the one whose marker the mapping recorded"*. Read in
order, the same word means two unrelated files.

Neither vocabulary yields, because both are the register's: `.coffret/root` is
the marker file EP-13 names, and KL-7 names its own term in full — an **explicit
key-lost marker**. So the fix is to use KL-7's full term at these three sites,
which is what `coffret-format` already does throughout (`control/ceiling.rs:56`,
`error.rs:386`). Bare *marker* then resolves to the root's marker in the two
crates where the collision arises, and nothing has to be coined.

### 7. Two doc comments that were written before EP-13 and EP-14 existed

**`fetch/entry_run.rs` step 5.** The step list at `entry_run.rs:42` cites
`(spec: EP-4, EP-10, EP-11)` and describes the discipline a placement keeps —
scratch name, mtime, hash, rename, marked present. The marker comparison happens
inside this step and it is not mentioned: `Placement::open`
(`fetch/placement.rs:139`) descends through `Destinations::reach`, and a root
that will not vouch for itself comes back as `Opened::RootRefused`, which
`range_read.rs:154` turns into `FetchError::RefusedRoot` for a single-Entry
fetch. Add EP-13 to step 5's rule list and say what it produces here, including
the difference a reader needs: for one Entry this refusal *fails* the fetch,
where `fetch_folders` reports the same refusal once per mapping and places the
rest.

**`routes/upload/receive.rs`.** The comment at `receive.rs:23` says the refusals
that are about one file are *"its name is not an Entry Path, the Library holds it
inside a Pack, this device could not write it"*. That third clause reads as an
I/O failure and folds three refusals under it, none of which is one: a name
coffret keeps for itself, refused by name before any disk is reached
(spec: EP-11, EP-14); a mapped root that will not vouch for itself, refused as
that root is opened (spec: EP-13); and a descent through something that is not a
real folder of that root, refused where the descent meets it (spec: EP-4,
EP-11). `routes/upload/mod.rs:100` now enumerates all of them under *"Refused
before anything lands"*. Name the three distinctly here, in `receive`'s own voice
— the order the function runs them in — and point at that enumeration for the
whole set rather than restating a count, so the two cannot drift apart.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] A folder whose Entry Path carries the reserved component has no local files
      of this device's, and a name reserved at any depth is stepped over whatever
      stands at it, proven by
      `grep -rq "fn nothing_under_the_management_area_is_listed_as_a_local_file" backend/crates/apps/coffret-device/src/add/`
      — the case asserts `added_locally` reports neither the marker nor an
      ordinary file standing at the reserved name.
- [x] The registration layer reads both spellings of a symbolic link at a
      reserved name, so a link is the typed verdict rather than a local I/O
      failure:
      `grep -rq "Errno::MLINK" backend/crates/apps/coffret-device/src/mapping/`.
- [x] `coffret_format::Error::EntropyUnavailable` carries the value the entropy
      source produced rather than its message, and the cause chain reaches it:
      `grep -rq "getrandom::Error" backend/crates/domain/coffret-format/src/` and
      `grep -rq "fn an_entropy_failure_carries_what_the_source_reported" backend/crates/domain/coffret-format/src/`.
- [x] The two cases arranging a root with no management area are named after that
      arrangement, and no spelling of the old name is left behind:
      `grep -rq "fn a_root_with_no_management_area_refuses_the_reach" backend/crates/domain/coffret-usecase/src/destinations_conformance/`,
      `grep -rq "fn a_root_with_no_management_area_places_nothing_and_reports_the_mapping" backend/crates/domain/coffret-usecase/tests/`,
      and `! grep -rq "a_root_with_no_marker" backend/crates/domain/coffret-usecase/`.
- [x] The destinations conformance suite takes the root as the subject of *vouch*
      rather than the mapping:
      `! grep -rq "the mapping cannot vouch for" backend/crates/domain/coffret-usecase/src/`.
- [x] The Keyring's marker is named by KL-7's own term wherever it stood beside a
      mapped root's marker:
      `! grep -rq "one marker locks every Entry" backend/crates/` and
      `grep -rq "key-lost marker" backend/crates/apps/coffret-device/src/`.
- [x] A single-Entry fetch's step list names the rule that refuses a mapped root:
      `grep -rq "EP-13" backend/crates/domain/coffret-usecase/src/fetch/entry_run.rs`.
- [x] The upload route's per-file refusals are named distinctly where one part is
      taken in, rather than folded under an I/O failure:
      `grep -rq "EP-14" backend/crates/apps/coffret-server/src/routes/upload/receive.rs`.
- [x] Existing backend, frontend and interoperability checks continue to pass —
      the destinations, fetch, spool, sync and freeze conformance suites, the
      router tests, the device-layer upload cases, and `make interop` included.

## Out of scope

- **Confirming the `EMLINK` arm on a BSD-family host.** Linux reports `ELOOP`
  for a symbolic link at either name, so no case in this suite drives the
  `EMLINK` arm; only a host that spells it `EMLINK` can exercise it. The arm is
  added here because the placement side already reads both spellings and the two
  sides must agree; confirming it on such a host belongs with the other
  on-hardware checks this group of changes has accumulated, not with this one.

- **Carrying the mapping's Library-side prefix in a refusal** so a person can
  tell which mapping was refused. That is a payload change to
  `FetchError::RefusedRoot` and to every site that constructs one, and it is its
  own change.
- **Making a drop into a refused root fail the request once** instead of refusing
  every part of it. A behaviour change in the upload route, with its own
  reasoning about what a browser reads mid-request; not a cleanup.
- **Dropping `derive(PartialEq, Eq)` from `Surfaced` and `FindingReason`.** Every
  conformance comparison against them would have to be rewritten.
- **`FetchError`'s two senses of `component`.**
  `UnmaterializablePath { component: Option<PathBuf> }` is the local folder a
  descent stopped at and renders as `descent=`
  (`fetch/fetch_error.rs:493`), while `ReservedComponent { component: String }`
  is a component of an Entry Path (`fetch_error.rs:154`). Renaming the first to
  read as a local folder is right, and it is a public field rename reaching
  `fetch_error.rs`, `fetch/translate.rs`, `coffret-device/src/error.rs` and
  `coffret-server/src/api_error/tests.rs` — its own change, alongside the rest of
  `DescentError`'s `path` / `component` crossing.
- **`MalformedMarker::NotText` discarding the `Utf8Error`**
  (`coffret-usecase/src/root_marker.rs:103`). Deliberately not carried. What the
  value holds is a byte offset into a file out of somebody's folder, and this
  module has already decided what such a file contributes: `defect()`
  (`root_marker.rs:144`) says the content itself never reaches an event and only
  which of the three ways it failed does. `TooLong { read }` carries a length
  because the length *is* what refused the file; a UTF-8 offset refuses nothing a
  caller acts on and distinguishes no failure `defect()` does not already name
  (spec: EL-1, EL-3), and putting it in the cause chain would cross a boundary
  with no defined log-safe rendering (spec: EL-4). Recorded here rather than
  changed, so the next reviewer of this enum finds the answer instead of the
  question.
- **`"temporary file"` → this repository's `scratch` vocabulary.** 87 occurrences
  across 37 files under `backend/` (106 across 44 repo-wide, counting `docs/` and
  `frontend/`), including `scratch.rs`'s own doc comments. Too large to ride
  alongside anything, and a partial rename is worse than none: its own PR, whose
  whole diff is that one substitution and the handful of sentences it forces.
- **Citing OC-8 where a doc comment cites OC-6** for idempotent local cleanup.
  OC-8 is the rule for it — *"Removing a local file this device wrote for its own
  purposes … is idempotent"* — and OC-8 is cited nowhere in `backend/` or
  `frontend/` today, while OC-6 (untrashed removals on Storage) is cited 36
  times across 29 files, about 22 of those citations about local files. It spans
  four crates, including `Cargo.toml` comments, and a handful of further sites are
  a judgement call (a *catalog row* dropped twice is neither rule squarely:
  `index.rs:248`, `index_conformance/device_state.rs:330` and `:388`,
  `sync_conformance/interruption.rs:176`/`:190`/`:224`/`:398`). Its own PR, where
  the judgement calls can be argued in one place.
- **Concept-document changes**: the `refuse` / `decline` doublet in the Entry
  Path concept's Collocations, naming the EP-13 verdict a "refused root" in the
  Library concept, and documenting the two shapes of *vouch*. Nothing under
  `docs/concepts/` or `docs/spec/` moves in this change; item 4 above fixes two
  strings in a test file and no prose about the term.
- **`receive_file.rs`'s `redundant explicit link target` rustdoc warnings.**
- Any migration or compatibility read. Backward compatibility is not a goal of
  this project.
