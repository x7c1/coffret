---
status: completed
pipeline_phase: null
plan: null
base_ref: null
perspectives: [completeness, clarity, rust-module-structure, error-type-design, concept-alignment]
max_refine_rounds: 3
retries_remaining: 1
check_command: "make check && ! grep -rq 'EntryPath::nfc' backend/crates && grep -rq 'pub fn parse' backend/crates/domain/coffret-model/src/entry_path* && grep -rq 'MalformedEntryPath' backend/crates/domain/coffret-model/src && grep -rq 'MalformedEntryPath' backend/crates/domain/coffret-format/src && ! grep -rq 'fn defect_in' backend/crates/apps/coffret-server/src && ! grep -rq 'component == \"\\.\\.\"' backend/crates/domain/coffret-usecase/src/fetch && grep -rq 'every_shape_ep_2_excludes_cannot_be_parsed' backend/crates/domain/coffret-model/src && grep -rq 'a_stored_path_with_a_shape_ep_2_excludes_is_refused' backend/crates/domain/coffret-model/src && grep -rq 'a_name_that_starts_with_a_dot_is_a_name' backend/crates/domain/coffret-model/src && grep -rq 'a_path_below_another_is_a_path' backend/crates/domain/coffret-model/src && grep -rq 'an_entry_path_with_a_shape_ep_2_excludes_is_rejected' backend/crates/domain/coffret-format/src/meta && grep -rq 'an_entry_path_with_a_shape_ep_2_excludes_is_rejected' backend/crates/domain/coffret-format/src/control/journal_record && grep -rq 'an_entry_path_with_a_shape_ep_2_excludes_is_rejected' backend/crates/domain/coffret-format/src/control/index_snapshot && grep -rq 'a_derived_from_path_with_a_shape_ep_2_excludes_is_rejected' backend/crates/domain/coffret-format/src/meta && grep -rq 'a_row_whose_path_has_a_shape_ep_2_excludes_makes_the_catalog_unreadable' backend/crates/gateway/coffret-sqlite-index/tests && grep -rq 'a_query_path_with_a_shape_ep_2_excludes_is_a_bad_path' backend/crates/apps/coffret-server && grep -rq 'a_mapping_prefix_with_more_than_one_component_is_refused' backend/crates/apps/coffret-device/src && grep -rq 'a_prefix_that_is_not_an_entry_path_is_refused_before_the_passphrase_is_asked' backend/crates/apps/coffret-cli && grep -q 'which part' docs/spec/entry-path/README.md"
assignee: null
branch: task/0904-1007-make-the-entry-path-shape-a-type-invariant
created_at: 2026-09-04T10:07:00Z
updated_at: 2026-09-04T12:10:24Z
---

# refactor(model): make the shape an Entry Path has an invariant of `EntryPath`

## Overview

`EntryPath` in `backend/crates/domain/coffret-model/src/entry_path.rs` makes
one of the two rules an Entry Path is held to an invariant of the type: every
`EntryPath` is NFC (spec: EP-1), and the two constructors say which side of the
Library boundary the text came from — `nfc` composes text from outside, and
`stored` refuses text the Library already holds when it is not NFC. The other
rule, the *shape* (spec: EP-2: non-empty, relative to the Library root, no
empty / `.` / `..` component, no leading or trailing `/`, no NUL, `/` the only
separator), is not the type's. Neither constructor looks at it, so a value of
the type can be `..`, `/etc/passwd`, `a//b`, or empty, and every place that
relies on the shape has to check it again — or forgets to:

- **Three separate implementations of EP-2, none of them in the type.** The
  server checks it on every `?path=` in
  `backend/crates/apps/coffret-server/src/entry_query.rs` (`shaped` /
  `defect_in`, 83-107); the fetch translates an `EntryPath` it was *already
  handed* and re-checks each component in
  `backend/crates/domain/coffret-usecase/src/fetch/translate.rs` (322-329,
  answering `FetchError::UnmaterializablePath`); and the device holds a
  mapping's prefix to a one-component version in
  `backend/crates/apps/coffret-device/src/name_defect.rs` (16-30), which the
  Library directory name in `library_dir.rs` shares.
- **Paths that skip all three.** The CLI's `fetch --under`, `fetch --entry`
  (`backend/crates/apps/coffret-cli/src/fetch.rs:38,45`) and `freeze --under`
  (`freeze.rs:41`) call `EntryPath::nfc` and pass the result straight to the
  usecase, so `..` or a trailing `/` reaches the catalog query and the fetch
  and is either answered with "nothing matched" or refused deep inside a run.
- **Stored text is never checked for shape.** A meta section
  (`backend/crates/domain/coffret-format/src/meta/stored_path.rs`), a Journal
  record or Index Snapshot entry (`control/wire_catalog_entry.rs`), a
  `derived_from` reference, and every catalog row
  (`backend/crates/gateway/coffret-sqlite-index/src/rows.rs`, via
  `EntryPath::stored`) is checked for NFC and nothing else. A control object
  carrying `../x` as an Entry Path decodes, applies to the Index, and comes out
  of `entries_under` as a path a fetch then has to catch. EP-1's sub-bullet
  states that a stored path outside the rule is *malformed* — a Container that
  does not open, a control object that does not decode, a catalog that is
  unreadable and rebuilt — and that is the answer EP-2 needs too.

Make EP-2 the type's, so that an `EntryPath` cannot exist in a shape the rule
excludes, and remove every re-check that the invariant makes unreachable.

### 1. The type

- Replace `EntryPath::nfc` with **`EntryPath::parse(text) -> Result<Self>`**:
  compose to NFC first, then check the shape, in that order (composing never
  creates or removes a `/`, a NUL, or an empty component, so the order matters
  only for the text a refusal quotes: it quotes what the caller typed). Also
  implement `FromStr` delegating to it, so a literal in a test reads as
  `"albums/a.jpg".parse()`. **There is no infallible constructor from
  arbitrary text**, in production or for tests: a test helper that unwraps
  `parse` in a crate's existing `testing` / `support` module is fine, a public
  `EntryPath::literal` is not.
- **`EntryPath::stored(text)`** keeps refusing non-NFC text as
  `Error::UnnormalizedEntryPath`, and now also refuses the shape, never
  normalizing. A stored path is checked for canonical form *and* shape; text
  from outside is normalized *then* checked for shape. Both paths through the
  type end in the same shape check, written once.
- **A new `coffret_model::Error::MalformedEntryPath { path: String, defect:
  PathDefect }`**, with `PathDefect` an enum (its own module) naming which part
  went: empty, holds a NUL, begins with a separator, ends with a separator,
  holds an empty component, holds a `.` or `..` component. `Display` on the
  defect says it in words a person reading a `400` can act on (the server's
  current messages in `defect_in` are the right register); the separators are
  looked at before the components, so a path with one on either end is told
  about that rather than about the empty component it leaves behind. The path
  travels in the value for the same reason `UnnormalizedEntryPath` carries it,
  and the `Redacted` rendering keeps it out of a log line the same way
  (`path_len`, not the path).
- **Composition is infallible.** Add `EntryPath::below(&self, relative:
  &EntryPath) -> EntryPath` (or a name that reads better at the call sites):
  two shaped paths joined by `/` are a shaped path, so no re-parse is owed.
  Use it where a path is currently rebuilt by `EntryPath::nfc(format!("{}/{}",
  …))` — `backend/crates/apps/coffret-server/src/routes/upload/under.rs`,
  `walk_mappings::entry_path` in
  `backend/crates/domain/coffret-usecase/src/local_scan/walk_mappings.rs`,
  `child_path` in `backend/crates/apps/coffret-device/src/folder_paths.rs`,
  and wherever else the grep finds it. A child *name* off a filesystem or the
  catalog is `parse`d as the one-component path it is and then joined.
- Keep `top_level`, `is_under`, `as_str`, `Display`, and the derived ordering
  as they are; nothing about EP-3 changes.

### 2. The boundaries, one by one

- **Scan** (`walk_mappings.rs`, 127-139): a directory entry's name goes
  through `parse`. A refusal cannot happen on a name `read_dir` returns
  (no `/`, no NUL, never `.` or `..`), and the code must still handle the
  `Result` honestly rather than `expect` it: report it as the same
  `LocalError::UnrepresentableName` a non-UTF-8 name gets, with the variant's
  doc widened from "not UTF-8" to "not a name the Library can hold". The
  scratch-name and other-mapping checks that follow read the parsed path.
- **Server** (`entry_query.rs`, `routes/upload/receive.rs`, `under.rs`):
  `shaped` becomes `EntryPath::parse(text).map_err(…)` into
  `ApiError::bad_path` carrying the defect's words; `defect_in` and its tests
  leave the server (the tests move to the model). The `entry()` / `folder()`
  split — empty means the root for a folder and is `bad_path` for an Entry —
  stays where it is, because that is a route's decision and not the type's.
  `api_error::bad_path` may take the defect rather than a `&str` if that reads
  better; the wire shape (`400`, code `bad_path`, a message beginning "that
  is not an Entry Path") does not change, since the explorer's `errors`
  handling and the router tests read it.
- **Fetch** (`fetch/translate.rs`, 322-329): drop the per-component
  re-check; only the prefix strip remains, and `UnmaterializablePath {
  component: None }` is now reached by exactly one path shape — a path
  standing at exactly a mapping's prefix (or outside it), which `translate`
  cannot place under that mapping. Rewrite the variant's doc and `Display`
  (`fetch/fetch_error.rs`, ~108-120 and ~284-295) so they no longer list EP-2
  shapes as ways to get there; the confinement tests in
  `backend/crates/domain/coffret-usecase/tests/fetch_confinement.rs` and the
  conformance cases that fed `..` paths in can no longer build such a path and
  are removed or reduced to the prefix case, whichever each one was really
  about.
- **Device** (`mapping/mod.rs` `entry_path`, ~121-135; `name_defect.rs`;
  `error.rs` `MalformedMappingPrefix` / `NameDefect`): a mapping's prefix goes
  through `EntryPath::parse` (EP-1 then EP-2) and then the one rule that is the
  device's own — exactly one component (spec: EP-9), i.e. `top_level()` is
  the whole path. `MalformedMappingPrefix` then has two causes, the model's
  `MalformedEntryPath` and "more than one component"; shape it so both are
  reported in words (carry the model error as a cause for the first, per the
  crate's error conventions) and drop the device-side re-implementation of
  empty / `.` / `..` / separator for the prefix. The `\` and control-character
  refusals `name_defect::defect_in` also makes are *not* EP-2 rules — a
  backslash is an ordinary character in an Entry Path and `/` is the only
  separator — so they stop applying to the prefix; they stay for the Library
  directory name in `library_dir.rs`, which is a directory name on this device
  and not an Entry Path. `name_defect.rs`'s module doc says it serves two
  callers; make it true again.
- **CLI** (`fetch.rs`, `freeze.rs`): `--under` and `--entry` go through
  `parse`, and a refusal is reported as the command's own usage error
  **before** the Passphrase is asked for — nobody should type a secret to be
  told their path had a trailing slash.
- **Format** (`meta/stored_path.rs`, and the `derived_from` path beside it):
  map the model's `MalformedEntryPath` to a new
  `coffret_format::Error::MalformedEntryPath { field }`, a sibling of
  `UnnormalizedEntryPath` naming the field and not the path, on the rule that
  enum states. Every decoder that reaches `stored_path` inherits it: meta
  section (`original_path`, `derived_from.original_path`), Journal record and
  Index Snapshot entry tables (`path`, `derived_from.original_path`).
- **SQLite** (`rows.rs`): nothing to change in code — `EntryPath::stored`
  already routes through `unreadable_model` — but the refusal has to be
  proven: a catalog row whose `path` is `../x` makes the catalog
  `UnreadableCatalog`, the same verdict a non-NFC row gets.
- **Interop generator** (`backend/crates/apps/coffret-interop/src/generate/`)
  and every other production call of `EntryPath::nfc` (see the grep: `folder.rs`,
  `classify.rs`, `findings.rs`, `browse/folders.rs`, the `*_error.rs` Display
  tests, `path_prefix.rs`, `container_writer.rs`, `container_footprint.rs`,
  `container_outline.rs`, `freeze/segment.rs`, `freeze/spool.rs`, …) move to
  `parse` or `below`. Where the text is a literal the crate itself wrote, an
  `expect` with a message saying it is a literal is acceptable in production
  only if there is no way to state it as a constant of the type; prefer
  restructuring so the text never leaves the type.

### 3. Tests that fix the rule

Use these names (the check command greps for them, anchored to directories so
refine may split files):

- `coffret-model`, `entry_path` tests: `every_shape_ep_2_excludes_cannot_be_parsed`
  (the eight shapes the server's test lists today, each yielding
  `MalformedEntryPath` with the expected `PathDefect`),
  `a_stored_path_with_a_shape_ep_2_excludes_is_refused` (the same shapes
  through `stored`, refused as malformed and never normalized — a decomposed
  *and* malformed path is refused for whichever the type checks first, and the
  test says which), `a_name_that_starts_with_a_dot_is_a_name` (`.hidden`,
  `...three` parse), `a_path_below_another_is_a_path` (`below` of two parsed
  paths equals the parse of their joined text, including when the top is one
  component).
- `coffret-format`: `an_entry_path_with_a_shape_ep_2_excludes_is_rejected`
  in `meta/rejection_tests.rs`, `control/journal_record/rejection_tests.rs`,
  and `control/index_snapshot/rejection_tests.rs` (tamper the path to `../x`
  the way the NFC cases tamper it; expect `MalformedEntryPath` naming the
  field), and `a_derived_from_path_with_a_shape_ep_2_excludes_is_rejected` in
  `meta/rejection_tests.rs`.
- `coffret-sqlite-index`, `tests/`:
  `a_row_whose_path_has_a_shape_ep_2_excludes_makes_the_catalog_unreadable`
  (write the row with `rusqlite` the way `tests/schema.rs` builds files, then
  read through the port and expect `IndexError::UnreadableCatalog`).
- `coffret-server`, router tests:
  `a_query_path_with_a_shape_ep_2_excludes_is_a_bad_path` (`?path=../x` on
  `/api/list` answers `400` with code `bad_path` and a message naming the
  `..` component).
- `coffret-device`: `a_mapping_prefix_with_more_than_one_component_is_refused`
  (`albums/2026` is refused as `MalformedMappingPrefix`; `albums` is not; and
  a prefix with a backslash in it is accepted).
- `coffret-cli`, `tests/`:
  `a_prefix_that_is_not_an_entry_path_is_refused_before_the_passphrase_is_asked`
  (`fetch --under 'albums/'` with `--passphrase-stdin` and an empty stdin exits
  non-zero naming the trailing separator, and the stderr does not mention the
  Passphrase; the setup harness in `tests/support` shows how a Library is
  staged for a CLI test).

Existing tests that built an `EntryPath` from a malformed literal to prove a
downstream refusal (search the conformance suites and `fetch_confinement.rs`
for `..`, `//`, and trailing `/`) are not converted to `expect`; each is either
deleted, because the type now proves it, or rewritten to the case it was really
about.

### 4. The spec

`docs/spec/entry-path/README.md`, EP-2: add a sub-bullet in the form of EP-1's,
stating the boundary — text from outside the Library that is not in this shape
is refused, told which part of the shape it fails (the check command greps
the spec for the words "which part"), and a stored path outside it
is malformed: the Container does not open, the control object does not decode,
and the device catalog is unreadable and rebuilt from Storage (RV-5). The rule
text itself does not change. Do not edit `docs/concepts/` in this task; if the
Entry Path concept needs a sentence, report it in the PR description.

### Out of scope

- The TypeScript format implementation. It already refuses non-NFC paths
  (`unnormalized_entry_path`) and the interop suite exchanges no malformed
  path, so nothing on that side changes; `make interop` (inside `make check`)
  must stay green.
- The byte extent of an Entry (`offset` / `size`), the aggregate constructors
  of the control objects, and the SQLite integer range — each is its own
  change, and this one must not touch `EntryMetadata`'s fields.
- Any change to the storage format or to what an existing Library can hold: a
  path the shape excludes was never producible by a scan, was refused by the
  server, and only the type stood between it and the catalog on the CLI
  paths, so refusing it on read narrows nothing the spec allowed.

## Acceptance criteria

### Automated (pipeline-verified)

- [x] `EntryPath::nfc` no longer exists; `EntryPath::parse` (fallible,
      normalize then shape) and `FromStr` replace it, and `EntryPath::stored`
      refuses the shape as well as non-NFC text, never normalizing
- [x] `coffret_model::Error::MalformedEntryPath { path, defect }` with a
      `PathDefect` naming which part of EP-2 failed; the path is redacted from
      log renderings the way `UnnormalizedEntryPath`'s is
- [x] `coffret_format::Error::MalformedEntryPath { field }` is what a meta
      section, a Journal record, and an Index Snapshot decode to when an entry
      path or a `derived_from` path is outside the shape
- [x] The server's `defect_in` and the fetch's per-component re-check are gone;
      the server answers `400` / `bad_path` from the model's defect, and the
      fetch's `UnmaterializablePath { component: None }` is reached only by the
      prefix case
- [x] A mapping prefix is an `EntryPath` of exactly one component; the
      device no longer re-implements EP-2 for it, and the Library directory
      name keeps its own rule
- [x] `fetch --under`, `fetch --entry`, and `freeze --under` refuse a
      malformed path before asking for the Passphrase
- [x] A catalog row with a path outside the shape makes the catalog
      `UnreadableCatalog`
- [x] The tests named in section 3 exist under the directories the check
      command greps
- [x] EP-2 in `docs/spec/entry-path/README.md` carries the boundary sub-bullet
- [x] `make check` (backend fmt / build / test / clippy, frontend, interop) is
      green
